//! B6's Agent prerequisite: real Daemon, fake Claude and its recorded Terminal input.
#[path = "support/rupd_harness.rs"]
mod rupd_harness;

use base64::{Engine, engine::general_purpose::STANDARD};
use contracts::EventData;
use rpc::{Client, RpcError};
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

const BOUND: Duration = Duration::from_secs(15);

struct Run {
    daemon: rupd_harness::Rupd,
    dir: tempfile::TempDir,
    data: tempfile::TempDir,
    user: Arc<Client>,
    child: String,
    door: Option<String>,
    terminal: Value,
    attempt: Value,
}

impl Run {
    async fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let fake = dir.path().join("claude");
        std::fs::write(&fake, r#"#!/usr/bin/env python3
import json, os, socket, subprocess, sys, threading, time, tty
if sys.argv[1:2] == ['--version']:
    print('2.1.300 (Claude Code)'); sys.exit(0)
if sys.argv[1:3] == ['plugin', 'list']:
    print('[]'); sys.exit(0)
if sys.argv[1:3] == ['plugin', 'test']:
    print('Q11 runtime hook closure: 1 pass'); sys.exit(0)
tty.setraw(0)
print('\033]0;✳ Ready\007', flush=True)
settings = json.load(open(sys.argv[sys.argv.index('--settings') + 1]))
def hook(event, fields):
    fields['hook_event_name'] = event
    for group in settings['hooks'][event]:
        for item in group['hooks']:
            subprocess.run(item['command'], shell=True, input=json.dumps(fields), text=True, check=True)
hook('SessionStart', {'session_id':'00000000-0000-4000-8000-000000000006','source':'resume' if '--resume' in sys.argv else 'startup'})
# B50: Claude Code listens on its messaging socket, lists its session with a token, records each
# message it takes up in its transcript and takes a turn on it.
def flag(name): return sys.argv[sys.argv.index(name) + 1]
agent = settings['hooks']['Stop'][0]['hooks'][0]['command'].split()[-1].strip("'")
sessions = os.path.join(os.environ.get('CLAUDE_CONFIG_DIR', os.getcwd() + '/config'), 'sessions')
transcript = os.path.join(os.getcwd(), 'transcript-' + agent + '.jsonl')
token = 'peer-token-' + str(os.getpid())
os.makedirs(sessions, exist_ok=True)
server = socket.socket(socket.AF_UNIX)
server.bind(flag('--messaging-socket-path'))
server.listen()
json.dump({'pid': os.getpid(), 'messagingSocketPath': flag('--messaging-socket-path')}, open(sessions + '/%d.json' % os.getpid(), 'w'))
json.dump({'peerToken': token}, open(sessions + '/%d.abc.key' % os.getpid(), 'w'))
def take(conn):
  try:
    lines = conn.makefile()
    if json.loads(lines.readline()).get('token') != token: return
    seen = set()
    for line in lines:
        msg = json.loads(line)
        # A msg_id taken up before is dropped, as Claude Code drops it.
        if msg['msg_id'] in seen: continue
        # The fixture's Claude never records a Decision note: it stays sent, not delivered.
        if os.path.isfile('decision-notice-fixture') and '[decision ' in msg['message']['content']: continue
        seen.add(msg['msg_id'])
        with open('received-' + agent + '.jsonl', 'a') as out: out.write(line)
        with open(transcript, 'a') as out: out.write(json.dumps({'type': 'user', 'origin': {'kind': 'peer', 'from': msg['from'], 'msg_id': msg['msg_id']}}) + '\n')
        hook('PostToolUse', {'transcript_path': transcript})
        hook('Stop', {'transcript_path': transcript})
  except BaseException: pass
def watch():
  try:
    while True:
        conn, _ = server.accept()
        threading.Thread(target=take, args=(conn,), daemon=True).start()
  except BaseException: pass
threading.Thread(target=watch, daemon=True).start()
if '--resume' not in sys.argv:
    pending = b''
    while not pending.endswith(b'\r'):
        pending += os.read(0, 1)
    text = pending.decode().replace('\x1b[200~','').replace('\x1b[201~','').removeprefix('Follow this instruction: ').strip()
    hook('UserPromptSubmit', {'prompt':text})
hook('Stop', {})
print('READY', flush=True)
pending = b''
while True:
    data = os.read(0, 1)
    if not data: break
    if data == b'\x01':
        call = json.load(open('door-call.json'))
        conn = socket.socket(socket.AF_UNIX)
        conn.connect(os.environ['RUPD_SOCKET'])
        conn.sendall((json.dumps({'jsonrpc':'2.0','id':1, **call}) + '\n').encode())
        reply = json.loads(conn.makefile('r').readline())
        conn.close()
        with open('door-reply.tmp', 'w') as out: json.dump(reply, out)
        os.rename('door-reply.tmp', 'door-reply.json')
        continue
    if data == b'\x02':
        def call():
            with socket.socket(socket.AF_UNIX) as client:
                client.connect('rupd.sock')
                stream = client.makefile('rwb')
                request = json.load(open('peer-request'))
                if isinstance(request, list):
                    for frame in request: stream.write(json.dumps(frame).encode() + b'\n')
                    stream.flush()
                    replies = []
                    for frame in request:
                        reply = json.loads(stream.readline())
                        replies.append(reply)
                        if reply.get('id') == 111: open('peer-barrier', 'w').write('scheduled')
                    with open('peer-result', 'w') as result: json.dump(replies, result)
                else:
                    stream.write(json.dumps(request).encode() + b'\n')
                    stream.flush()
                    with open('peer-result', 'w') as result: result.write(stream.readline().decode())
        threading.Thread(target=call, daemon=True).start()
        continue
    print('BARRIER' if data == b'\x00' else 'BYTE:' + data.hex(), flush=True)
    if data != b'\x00': pending += data
    if data == b'\r' and (os.path.isfile('order-submit') or os.path.isfile('decision-notice-fixture')):
        text = pending.decode().replace('\x1b[200~','').replace('\x1b[201~','').removeprefix('Follow this instruction: ').strip()
        pending = b''
        if os.path.isfile('decision-notice-fixture') and '[decision ' in text:
            continue
        deadline = time.monotonic() + 10
        while 'occupying steer' in text and os.path.isfile('background-gate'):
            if time.monotonic() >= deadline: raise RuntimeError('background Steer was not released')
            time.sleep(0.005)
        hook('UserPromptSubmit', {'prompt':text})
        hook('Stop', {})
        print('SUBMITTED', flush=True)
"#).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let daemon = launch(dir.path(), data.path());
        let user = Arc::new(Client::connect(&daemon.socket).await.unwrap());
        let mut run = Self {
            daemon,
            dir,
            data,
            user,
            child: String::new(),
            door: None,
            terminal: Value::Null,
            attempt: Value::Null,
        };
        run.spawn().await;
        run
    }

    async fn spawn(&mut self) {
        assert!(self.data.path().is_dir());
        let node = self
            .user
            .request(
                "agent.spawn",
                json!({"name":"child","cwd":self.dir.path(),"parent":self.door}),
            )
            .await
            .unwrap();
        self.child = node["id"].as_str().unwrap().to_owned();
        self.terminal = node["terminal_id"].clone();
        self.attempt = node["attempt"].clone();
        tokio::time::timeout(BOUND, async {
            loop {
                if self.screen().await.contains("READY") {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    async fn screen(&self) -> String {
        self.user
            .request("terminal.read", json!({"id":self.terminal,"after":0}))
            .await
            .unwrap()["text"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    async fn open(&self, kind: &str) -> (Value, tokio::task::JoinHandle<Result<Value, RpcError>>) {
        self.open_prompt(kind, "Same prompt?").await
    }

    async fn open_prompt(
        &self,
        kind: &str,
        prompt: &str,
    ) -> (Value, tokio::task::JoinHandle<Result<Value, RpcError>>) {
        let before = self.user.request("decision.list", ()).await.unwrap();
        let before = before.as_array().unwrap().first().map(|d| d["id"].clone());
        let caller = Arc::new(Client::connect(&self.daemon.socket).await.unwrap());
        if kind == "ask_user" {
            caller
                .request(
                    "daemon.identify",
                    json!({"actor":{"kind":"agent","id":self.child}}),
                )
                .await
                .unwrap();
        }
        let child = self.child.clone();
        let kind = kind.to_owned();
        let prompt = prompt.to_owned();
        let task = tokio::spawn(async move {
            if kind == "ask_user" {
                caller.request("agent.ask", json!({"question":prompt,"answers":[{"label":"Yes","description":"Proceed"}]})).await
            } else {
                let mut payload = if kind == "unreadable" {
                    json!({})
                } else if kind == "native" {
                    json!({"tool_name":"AskUserQuestion","tool_input":{"questions":[{"question":"Same prompt?","options":[{"label":"Yes"}]}]}})
                } else {
                    json!({"tool_name":"Bash","tool_input":{"command":prompt}})
                };
                payload["hook_event_name"] = json!("PermissionRequest");
                caller
                    .request("agent.permission", json!({"id":child,"payload":payload}))
                    .await
            }
        });
        let decision = tokio::time::timeout(BOUND, async {
            loop {
                let listed = self.user.request("decision.list", ()).await.unwrap();
                if let Some(decision) = listed.as_array().unwrap().first()
                    && Some(&decision["id"]) != before.as_ref()
                {
                    return decision.clone();
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        (decision, task)
    }

    async fn capture(&self, decision: &Value) -> Value {
        self.user
            .request("decision.capture", json!({"id":decision["id"]}))
            .await
            .unwrap()
    }

    async fn clear(&self) {
        self.user.request("agent.signal", json!({"id":self.child,"attempt":self.attempt,"payload":{"hook_event_name":"PostToolUse"}})).await.unwrap();
    }

    async fn write(&self, identity: Value, bytes: &[u8]) -> Result<Value, RpcError> {
        self.user
            .request(
                "terminal.write",
                json!({"id":self.terminal,"data":STANDARD.encode(bytes),"identity":identity}),
            )
            .await
    }

    async fn user_answer(&self, decision: &Value, kind: &str) {
        if kind == "native" {
            self.user
                .request(
                    "terminal.write",
                    json!({"id":self.terminal,"data":STANDARD.encode(b"u")}),
                )
                .await
                .unwrap();
            self.clear().await;
        } else {
            self.user.request("decision.answer", json!({"id":decision["id"],"answer":if kind == "permission" {"allow"} else {"Yes"},"proof":"0".repeat(64)})).await.unwrap();
        }
    }

    async fn guarded_answer(
        &self,
        decision: &Value,
        identity: Value,
        kind: &str,
    ) -> Result<Value, RpcError> {
        self.user.request("decision.answer", json!({"id":decision["id"],"identity":identity,"answer":if kind == "permission" {"allow"} else {"Yes"},"proof":"0".repeat(64)})).await
    }

    async fn resumed(&mut self) {
        let node = self
            .user
            .request("agent.resume", json!({"id":self.child}))
            .await
            .unwrap();
        self.terminal = node["terminal_id"].clone();
        self.attempt = node["attempt"].clone();
        tokio::time::timeout(BOUND, async {
            loop {
                if self.screen().await.contains("READY") {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    async fn paused(&self) {
        let arrival = self.dir.path().join("effect-gate.arrived");
        tokio::time::timeout(BOUND, async {
            loop {
                if arrival.is_file() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    fn pause(&self) {
        let arrival = self.dir.path().join("effect-gate.arrived");
        if arrival.exists() {
            std::fs::remove_file(arrival).unwrap();
        }
        std::fs::write(self.dir.path().join("effect-gate"), "before").unwrap();
    }

    fn release(&self) {
        std::fs::remove_file(self.dir.path().join("effect-gate")).unwrap();
    }

    async fn recorded_input(&self) -> String {
        // A NUL is a recording-handler barrier, not a prompt answer. Its receipt proves
        // the input thread and fake Claude consumed everything preceding the refusal.
        let before = self.screen().await.matches("BARRIER").count();
        let observer = Client::connect(&self.daemon.socket).await.unwrap();
        observer
            .request(
                "daemon.identify",
                json!({"actor":{"kind":"ext","id":"b6-observer"}}),
            )
            .await
            .unwrap();
        observer
            .request(
                "terminal.write",
                json!({"id":self.terminal,"data":STANDARD.encode(b"\0")}),
            )
            .await
            .unwrap();
        tokio::time::timeout(BOUND, async {
            loop {
                let recorded = self.screen().await;
                if recorded.matches("BARRIER").count() == before + 1 {
                    return recorded;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap()
    }

    async fn finish(self) {
        self.user
            .request("agent.stop", json!({"id":self.child}))
            .await
            .unwrap();
    }
}

fn launch(project: &Path, data: &Path) -> rupd_harness::Rupd {
    let rup = Path::new(env!("CARGO_BIN_EXE_rupd")).with_file_name("rup");
    assert!(rup.is_file(), "cargo build -p rup first");
    rupd_harness::start(
        Path::new(env!("CARGO_BIN_EXE_rupd")),
        project,
        true,
        |command| {
            command
                .env("TOKIO_WORKER_THREADS", "16")
                .env("RUPD_PROMPT_EFFECT_GATE", project.join("effect-gate"))
                .env("RUPD_ORDER_EFFECT_GATE", project.join("order-effect-gate"))
                .env("RUPD_TERMINAL_INPUT_GATE", project.join("input-gate"))
                .env("ROUNDUP_DATA_DIR", data)
                .env("ROUNDUP_CLAUDE_BIN", project.join("claude"))
                .env("ROUNDUP_RUP_BIN", rup)
                .env("CLAUDE_CONFIG_DIR", project.join("config"))
                .stderr(std::process::Stdio::null());
        },
    )
}

#[tokio::test]
async fn b6_a_valid_original_prompt_has_one_effect() {
    let run = Run::start().await;
    for kind in ["permission", "ask_user", "native"] {
        let (decision, waiting) = run.open(kind).await;
        let identity = run.capture(&decision).await;
        if kind == "native" {
            run.write(identity.clone(), b"v").await.unwrap();
            let err = run.write(identity, b"duplicate").await.unwrap_err();
            assert_eq!(err.code, rpc::code::CONFLICT);
            assert!(err.message.contains("original prompt"));
            tokio::time::timeout(BOUND, async {
                loop {
                    if run.screen().await.contains("BYTE:76") {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert_eq!(run.screen().await.matches("BYTE:").count(), 1);
            // H7: typing does not prematurely remove the native Decision Card.
            assert_eq!(
                run.user.request("decision.list", ()).await.unwrap()[0]["id"],
                decision["id"]
            );
            run.clear().await;
        } else {
            run.guarded_answer(&decision, identity.clone(), kind)
                .await
                .unwrap();
            let err = run
                .guarded_answer(&decision, identity, kind)
                .await
                .unwrap_err();
            assert_eq!(err.code, rpc::code::CONFLICT);
            assert!(err.message.contains("original prompt"));
        }
        let reply = waiting.await.unwrap().unwrap();
        if kind == "permission" {
            assert!(reply["output"].as_str().unwrap().contains("allow"));
        }
        if kind == "ask_user" {
            assert_eq!(reply["answer"], "Yes");
        }
    }
    run.finish().await;
}

#[tokio::test]
async fn b6_terminal_input_invalidates_identity_before_the_next_signal() {
    let run = Run::start().await;
    for kind in ["permission", "ask_user", "native"] {
        let (decision, waiting) = run.open(kind).await;
        let identity = run.capture(&decision).await;
        run.user
            .request(
                "terminal.write",
                json!({"id":run.terminal,"data":STANDARD.encode(b"u")}),
            )
            .await
            .unwrap();
        let unavailable = run
            .user
            .request("decision.capture", json!({"id":decision["id"]}))
            .await
            .unwrap_err();
        assert!(unavailable.message.contains("identity unavailable"));
        assert!(
            run.write(identity.clone(), b"old")
                .await
                .unwrap_err()
                .message
                .contains("original prompt")
        );
        if kind != "native" {
            assert!(
                run.guarded_answer(&decision, identity, kind)
                    .await
                    .unwrap_err()
                    .message
                    .contains("original prompt")
            );
        }
        run.clear().await;
        let _ = waiting.await.unwrap();
    }
    assert!(!run.recorded_input().await.contains("BYTE:6f"));
    let unavailable = run
        .user
        .request("decision.capture", json!({"id":"absent"}))
        .await
        .unwrap_err();
    assert!(unavailable.message.contains("unavailable"));
    run.finish().await;
}

#[tokio::test]
async fn b6_an_attempt_change_refuses_the_original_prompt_effect() {
    for kind in ["permission", "ask_user", "native"] {
        let mut run = Run::start().await;
        let (original, waiting) = run.open(kind).await;
        assert_eq!(original["id"], "d1");
        let identity = run.capture(&original).await;
        run.user
            .request("agent.stop", json!({"id":run.child}))
            .await
            .unwrap();
        let _ = waiting.await.unwrap();
        let attempt = run.attempt.clone();
        run.resumed().await;
        assert_ne!(run.attempt, attempt);
        let (replacement, waiting) = run.open(kind).await;
        assert_eq!(original["args"], replacement["args"]);
        assert!(
            run.write(identity.clone(), b"old")
                .await
                .unwrap_err()
                .message
                .contains("original prompt")
        );
        if kind != "native" {
            assert!(
                run.guarded_answer(&original, identity.clone(), kind)
                    .await
                    .unwrap_err()
                    .message
                    .contains("original prompt")
            );
        }
        run.clear().await;
        let _ = waiting.await.unwrap();
        assert!(!run.recorded_input().await.contains("BYTE:"));
        run.finish().await;
    }
}

#[tokio::test]
async fn b6_a_daemon_crash_refuses_a_live_original_even_with_d1_reused() {
    for kind in ["permission", "ask_user", "native"] {
        let mut run = Run::start().await;
        let (original, original_waiting) = run.open(kind).await;
        assert_eq!(original["id"], "d1");
        let identity = run.capture(&original).await;
        run.daemon.child.kill().unwrap();
        run.daemon.child.wait().unwrap();
        let old_reply = original_waiting.await.unwrap();
        if kind != "native" {
            assert!(old_reply.is_err());
        }
        run.daemon = launch(run.dir.path(), run.data.path());
        run.user = Arc::new(Client::connect(&run.daemon.socket).await.unwrap());
        run.resumed().await;
        let (replacement, waiting) = run.open(kind).await;
        assert_eq!(replacement["id"], "d1");
        assert_eq!(original["args"], replacement["args"]);
        let fresh = run.capture(&replacement).await;
        assert_ne!(identity["daemon"], fresh["daemon"]);
        assert!(
            run.write(identity.clone(), b"old")
                .await
                .unwrap_err()
                .message
                .contains("original prompt")
        );
        if kind != "native" {
            assert!(
                run.guarded_answer(&original, identity.clone(), kind)
                    .await
                    .unwrap_err()
                    .message
                    .contains("original prompt")
            );
        }
        // The Daemon run is an independent fence, even if Attempt and d1 coincide.
        let mut reused = identity;
        reused["attempt"] = fresh["attempt"].clone();
        assert!(
            run.write(reused, b"old")
                .await
                .unwrap_err()
                .message
                .contains("original prompt")
        );
        assert!(!run.recorded_input().await.contains("BYTE:"));
        run.clear().await;
        let _ = waiting.await.unwrap();
        run.finish().await;
    }
}

#[tokio::test]
async fn b6_a_paused_effect_refuses_after_the_user_answers_or_replaces_the_original() {
    for kind in ["permission", "ask_user", "native"] {
        for answer_effect in [false, true] {
            if kind == "native" && answer_effect {
                continue;
            }
            let run = Run::start().await;
            let (original, waiting) = run.open(kind).await;
            let identity = run.capture(&original).await;
            run.pause();
            let caller = Arc::new(Client::connect(&run.daemon.socket).await.unwrap());
            let params = if answer_effect {
                json!({"id":original["id"],"identity":identity,"answer":if kind == "permission" {"allow"} else {"Yes"},"proof":"0".repeat(64)})
            } else {
                json!({"id":run.terminal,"data":STANDARD.encode(b"old"),"identity":identity})
            };
            let effect = tokio::spawn(async move {
                caller
                    .request(
                        if answer_effect {
                            "decision.answer"
                        } else {
                            "terminal.write"
                        },
                        params,
                    )
                    .await
            });
            run.paused().await;
            run.user_answer(&original, kind).await;
            let _ = waiting.await.unwrap();
            let (replacement, waiting) = run.open(kind).await;
            assert_eq!(original["args"], replacement["args"]);
            run.release();
            let err = effect.await.unwrap().unwrap_err();
            assert_eq!(err.code, rpc::code::CONFLICT);
            assert!(err.message.contains("original prompt"), "{err:?}");
            assert!(!err.message.contains("observer"), "{err:?}");
            assert!(!run.recorded_input().await.contains("BYTE:6f"));
            assert_eq!(
                run.user.request("decision.list", ()).await.unwrap()[0]["id"],
                replacement["id"]
            );
            // The replacement still has its own usable identity.
            let fresh = run.capture(&replacement).await;
            if kind == "native" {
                run.write(fresh, b"v").await.unwrap();
                run.clear().await;
            } else {
                run.guarded_answer(&replacement, fresh, kind).await.unwrap();
            }
            waiting.await.unwrap().unwrap();
            run.finish().await;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn b6_checked_effects_exclude_prompt_replacement_under_parallel_load() {
    let mut runs = tokio::task::JoinSet::new();
    for _ in 0..8 {
        runs.spawn(checked_effect_before_replacement());
    }
    while let Some(result) = runs.join_next().await {
        result.unwrap();
    }
}

async fn checked_effect_before_replacement() {
    for kind in ["permission", "ask_user", "native"] {
        for answer_effect in [false, true] {
            if kind == "native" && answer_effect {
                continue;
            }
            let run = Run::start().await;
            let (original, waiting) = run.open(kind).await;
            let identity = run.capture(&original).await;
            std::fs::write(run.dir.path().join("effect-gate"), "effect").unwrap();
            let caller = Arc::new(Client::connect(&run.daemon.socket).await.unwrap());
            let params = if answer_effect {
                json!({"id":original["id"],"identity":identity,"answer":if kind == "permission" {"allow"} else {"Yes"},"proof":"0".repeat(64)})
            } else {
                json!({"id":run.terminal,"data":STANDARD.encode(b"v"),"identity":identity})
            };
            let effect = tokio::spawn(async move {
                caller
                    .request(
                        if answer_effect {
                            "decision.answer"
                        } else {
                            "terminal.write"
                        },
                        params,
                    )
                    .await
            });
            run.paused().await;
            // Submit replacement before releasing the physical write. The Signal's fold must
            // acquire the same locks, so it cannot invalidate midway through the old effect.
            use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
            let mut user = tokio::net::UnixStream::connect(&run.daemon.socket)
                .await
                .unwrap();
            let signal = json!({"jsonrpc":"2.0","id":1,"method":"agent.signal","params":{"id":run.child,"attempt":run.attempt,"payload":{"hook_event_name":"PostToolUse"}}});
            user.write_all(format!("{signal}\n").as_bytes())
                .await
                .unwrap();
            tokio::time::timeout(BOUND, async {
                loop {
                    if run.dir.path().join("effect-gate.racing").is_file() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert!(
                !effect.is_finished(),
                "effect left its pause before release"
            );
            // An observation window: a folded replacement would answer this socket.
            // The router's racing acknowledgement bounds its arrival, not a scheduler guess.
            let mut user = tokio::io::BufReader::new(user);
            let mut response = String::new();
            assert!(
                tokio::time::timeout(Duration::from_millis(50), user.read_line(&mut response))
                    .await
                    .is_err(),
                "{kind} answer_effect={answer_effect}: replacement completed while the original effect was paused: {response}"
            );
            run.release();
            effect.await.unwrap().unwrap();
            user.read_line(&mut response).await.unwrap();
            assert!(
                serde_json::from_str::<Value>(&response)
                    .unwrap()
                    .get("result")
                    .is_some(),
                "{response}"
            );
            let outcome = waiting.await.unwrap();
            if answer_effect {
                outcome.unwrap();
            }
            let (replacement, waiting) = run.open(kind).await;
            assert_eq!(original["args"], replacement["args"]);
            let recorded = run.recorded_input().await;
            assert_eq!(
                recorded.matches("BYTE:").count(),
                usize::from(!answer_effect)
            );
            assert!(run.capture(&replacement).await.is_object());
            run.clear().await;
            let _ = waiting.await.unwrap();
            run.finish().await;
        }
    }
}

#[tokio::test]
async fn b6_replacing_an_open_prompt_invalidates_its_captured_effect() {
    let run = Run::start().await;
    for kind in ["permission", "ask_user", "native"] {
        let (original, waiting) = run.open(kind).await;
        let identity = run.capture(&original).await;
        let (replacement, replacement_waiting) = run.open(kind).await;
        assert_eq!(original["args"], replacement["args"]);
        if kind != "native" {
            assert!(
                waiting
                    .await
                    .unwrap()
                    .unwrap_err()
                    .message
                    .contains("superseded")
            );
            assert!(
                run.guarded_answer(&original, identity.clone(), kind)
                    .await
                    .unwrap_err()
                    .message
                    .contains("original prompt")
            );
        } else {
            waiting.await.unwrap().unwrap();
        }
        assert!(
            run.write(identity, b"old")
                .await
                .unwrap_err()
                .message
                .contains("original prompt")
        );
        assert!(!run.recorded_input().await.contains("BYTE:"));
        assert!(run.capture(&replacement).await.is_object());
        run.clear().await;
        let _ = replacement_waiting.await.unwrap();
    }
    run.finish().await;
}

#[tokio::test]
async fn b6_an_unreadable_prompt_refuses_identity_and_effects() {
    let run = Run::start().await;
    let (original, waiting) = run.open("permission").await;
    let mut identity = run.capture(&original).await;
    run.clear().await;
    let _ = waiting.await.unwrap();
    let (unreadable, waiting) = run.open("unreadable").await;
    assert!(!unreadable["answerable"].as_bool().unwrap());
    let err = run
        .user
        .request("decision.capture", json!({"id":unreadable["id"]}))
        .await
        .unwrap_err();
    assert!(err.message.contains("identity unavailable"));
    // Even a fabricated identity with the current id cannot authorize an unreadable prompt.
    identity["decision"] = unreadable["id"].clone();
    let err = run.write(identity, b"old").await.unwrap_err();
    assert!(err.message.contains("identity unavailable"));
    assert!(!run.recorded_input().await.contains("BYTE:"));
    waiting.await.unwrap().unwrap();
    run.clear().await;
    run.finish().await;
}

#[tokio::test]
async fn b6_refusal_names_the_original_when_a_replacement_identity_is_unavailable() {
    let run = Run::start().await;
    for kind in ["permission", "ask_user", "native"] {
        let (original, waiting) = run.open(kind).await;
        let identity = run.capture(&original).await;
        run.user_answer(&original, kind).await;
        let _ = waiting.await.unwrap();
        let (replacement, waiting) = run.open(kind).await;
        run.user
            .request(
                "terminal.write",
                json!({"id":run.terminal,"data":STANDARD.encode(b"u")}),
            )
            .await
            .unwrap();
        let err = run.write(identity.clone(), b"old").await.unwrap_err();
        assert!(
            err.message
                .contains(&format!("prompt {}", original["id"].as_str().unwrap())),
            "{err:?}"
        );
        assert!(
            !err.message
                .contains(&format!("prompt {}", replacement["id"].as_str().unwrap())),
            "{err:?}"
        );
        if kind != "native" {
            let err = run
                .guarded_answer(&original, identity, kind)
                .await
                .unwrap_err();
            assert!(
                err.message
                    .contains(&format!("prompt {}", original["id"].as_str().unwrap())),
                "{err:?}"
            );
        }
        assert!(!run.recorded_input().await.contains("BYTE:6f"));
        run.clear().await;
        let _ = waiting.await.unwrap();
    }
    run.finish().await;
}

#[tokio::test]
async fn b6_an_order_completion_waits_for_its_steer_effect() {
    let (run, caller) = order_child().await;
    std::fs::write(run.dir.path().join("order-effect-gate"), "pause").unwrap();
    let child = run.child.clone();
    let mut completion = tokio::spawn(async move {
        caller
            .request(
                "agent.setOrder",
                json!({"id":child,"order":{"kind":"work","ask":"first order","limits":[]}}),
            )
            .await
    });
    tokio::select! {
        result = &mut completion => panic!("Order returned before its Steer effect: {result:?}"),
        arrived = tokio::time::timeout(BOUND, async {
            while !run.dir.path().join("order-effect-gate.arrived").is_file() { tokio::task::yield_now().await; }
        }) => { arrived.unwrap(); }
    }
    assert!(!completion.is_finished());
    std::fs::remove_file(run.dir.path().join("order-effect-gate")).unwrap();
    // This fake deliberately withholds submit: physically written bytes are uncertain,
    // never a successful Rail node and never an invitation to retry.
    let err = completion.await.unwrap().unwrap_err();
    assert_eq!(err.code, rpc::code::UNKNOWN_OUTCOME);
    assert!(err.message.contains("Order stored"), "{err:?}");
    assert!(err.message.contains("do not retry"), "{err:?}");
    assert!(run.recorded_input().await.contains("BYTE:"));
    run.finish().await;
}

async fn order_child() -> (Run, Arc<Client>) {
    let mut run = Run::start().await;
    run.user
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    let door = run
        .user
        .request(
            "rail.createWorkstream",
            json!({"name":"team","parent":null}),
        )
        .await
        .unwrap();
    let caller = Arc::new(Client::connect(&run.daemon.socket).await.unwrap());
    caller
        .request(
            "daemon.identify",
            json!({"actor":{"kind":"agent","id":door["id"]}}),
        )
        .await
        .unwrap();
    let child = caller
        .request(
            "agent.spawn",
            json!({"name":"ordered child","cwd":run.dir.path()}),
        )
        .await
        .unwrap();
    run.child = child["id"].as_str().unwrap().to_owned();
    run.terminal = child["terminal_id"].clone();
    run.attempt = child["attempt"].clone();
    tokio::time::timeout(BOUND, async {
        while !run.screen().await.contains("READY") {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    (run, caller)
}

async fn idle(run: &Run) {
    tokio::time::timeout(BOUND, async {
        loop {
            let tree = run.user.request("rail.tree", ()).await.unwrap();
            if tree
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["id"] == run.child && node["status"]["kind"] == "idle")
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

async fn order_result(
    run: &Run,
    caller: Arc<Client>,
) -> tokio::task::JoinHandle<Result<Value, RpcError>> {
    let child = run.child.clone();
    tokio::spawn(async move {
        caller
            .request(
                "agent.setOrder",
                json!({"id":child,"order":{"kind":"work","ask":"first order","limits":[]}}),
            )
            .await
    })
}

async fn recorded_bytes(run: &Run) -> Vec<u8> {
    run.recorded_input()
        .await
        .lines()
        .filter_map(|line| {
            line.trim_start()
                .strip_prefix("BYTE:")
                .and_then(|hex| u8::from_str_radix(hex.trim(), 16).ok())
        })
        .collect()
}

#[tokio::test]
async fn b6_an_order_effect_finishes_before_the_following_prompt() {
    let (run, caller) = order_child().await;
    std::fs::write(run.dir.path().join("order-submit"), "accept").unwrap();
    std::fs::write(run.dir.path().join("order-effect-gate"), "pause").unwrap();
    let mut completion = order_result(&run, Arc::clone(&caller)).await;
    tokio::select! {
        result = &mut completion => panic!("premature completion: {result:?}"),
        arrived = tokio::time::timeout(BOUND, async {
            while !run.dir.path().join("order-effect-gate.arrived").is_file() { tokio::task::yield_now().await; }
        }) => { arrived.unwrap(); }
    }
    assert!(!run.screen().await.contains("BYTE:"));
    std::fs::remove_file(run.dir.path().join("order-effect-gate")).unwrap();
    let node = completion.await.unwrap().unwrap();
    assert_eq!(node["work"]["ask"], "first order");
    idle(&run).await;
    caller
        .request(
            "agent.prompt",
            json!({"id":run.child,"text":"second prompt"}),
        )
        .await
        .unwrap();
    let bytes = String::from_utf8(recorded_bytes(&run).await).unwrap();
    assert_eq!(
        bytes,
        format!(
            "{}\x1b[200~first order\x1b[201~\r{}\x1b[200~second prompt\x1b[201~\r",
            agents::claude_code::LEAD_IN,
            agents::claude_code::LEAD_IN
        )
    );
    run.finish().await;
}

#[tokio::test]
async fn b6_a_refused_order_steer_reports_the_stored_order_without_a_write() {
    let (run, caller) = order_child().await;
    std::fs::write(run.dir.path().join("order-effect-gate"), "refuse").unwrap();
    let err = order_result(&run, caller).await.await.unwrap().unwrap_err();
    assert_eq!(err.code, rpc::code::NOT_ACCEPTED);
    assert!(err.message.contains("Order stored"), "{err:?}");
    assert!(err.message.contains("no bytes written"), "{err:?}");
    std::fs::remove_file(run.dir.path().join("order-effect-gate")).unwrap();
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    assert!(
        tree.as_array()
            .unwrap()
            .iter()
            .any(|node| node["id"] == run.child && node["work"]["ask"] == "first order")
    );
    assert!(recorded_bytes(&run).await.is_empty());
    run.finish().await;
}

#[tokio::test]
async fn b6_a_child_ending_after_order_bytes_never_authorizes_a_retry() {
    let (run, caller) = order_child().await;
    let completion = order_result(&run, caller).await;
    tokio::time::timeout(BOUND, async {
        while !run.screen().await.contains("BYTE:0d") {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!completion.is_finished());
    run.user
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    let err = completion.await.unwrap().unwrap_err();
    assert_eq!(err.code, rpc::code::UNKNOWN_OUTCOME);
    assert!(
        err.message.contains("bytes may have been written"),
        "{err:?}"
    );
    assert!(err.message.contains("do not retry"), "{err:?}");
}

#[tokio::test]
async fn b6_a_busy_child_reports_a_refused_order_steer() {
    let (run, caller) = order_child().await;
    run.user.request("agent.signal", json!({"id":run.child,"attempt":run.attempt,"payload":{"hook_event_name":"UserPromptSubmit"}})).await.unwrap();
    let err = order_result(&run, caller).await.await.unwrap().unwrap_err();
    assert_eq!(err.code, rpc::code::BUSY);
    assert!(err.message.contains("Order stored"), "{err:?}");
    assert!(err.message.contains("no bytes written"), "{err:?}");
    assert_eq!(ordered_ask(&run).await, "first order");
    assert!(recorded_bytes(&run).await.is_empty());
    run.finish().await;
}

#[tokio::test]
async fn b6_a_refused_prompt_effect_does_not_submit_the_users_input() {
    let run = Run::start().await;
    let (original, waiting) = run.open("native").await;
    let identity = run.capture(&original).await;
    run.user
        .request(
            "terminal.write",
            json!({"id":run.terminal,"data":STANDARD.encode(b"draft")}),
        )
        .await
        .unwrap();
    let before = run.recorded_input().await;
    let message = run
        .user
        .request(
            "message.send",
            json!({"to":run.child,"kind":"note","body":"wait for the user's input"}),
        )
        .await
        .unwrap();
    assert_eq!(message["status"], "held");
    let refused = run.write(identity, b"\r").await.unwrap_err();
    assert_eq!(refused.code, rpc::code::CONFLICT);
    let held = run
        .user
        .request("message.get", json!({"id":message["id"]}))
        .await
        .unwrap();
    assert_eq!(
        held["status"], "held",
        "a refused submit released the Message"
    );
    assert_eq!(held["reason"], "takeover");
    assert_eq!(
        run.screen().await.matches("BYTE:").count(),
        before.matches("BYTE:").count()
    );
    run.clear().await;
    let _ = waiting.await.unwrap();
    run.finish().await;
}

async fn notice_events(run: &Run) -> Client {
    let client = Client::connect(&run.daemon.socket).await.unwrap();
    tokio::time::timeout(BOUND, client.request("events.subscribe", ()))
        .await
        .unwrap()
        .unwrap();
    client
}

async fn failed_notice(
    run: &Run,
    events: &mut Client,
    decision: &Value,
    failure: &str,
    sent: Option<&contracts::message::Message>,
    bound: Duration,
) -> Result<Value, String> {
    let started = Instant::now();
    let marker = format!("[decision {}]", decision["id"].as_str().unwrap());
    let mut stage = sent.map_or_else(
        || "Decision opened (decision.list)".to_owned(),
        |message| format!("message.sent: {}", serde_json::to_value(message).unwrap()),
    );
    let observed = tokio::time::timeout(bound, async {
        loop {
            let event = events
                .next_event()
                .await
                .ok_or("subscription closed".to_owned())?;
            let (name, message) = match event.data {
                EventData::MessageSent(m) => ("message.sent", m),
                EventData::MessageHeld(m) => ("message.held", m),
                EventData::MessageDropped(m) => ("message.dropped", m),
                EventData::MessageDelivered(m) => ("message.delivered", m),
                _ => continue,
            };
            if !message.body.contains(&marker) {
                continue;
            }
            stage = format!("{name}: {}", serde_json::to_value(&message).unwrap());
            eprintln!("{failure}: {:?}; {stage}", started.elapsed());
            if name == "message.delivered" && message.to == contracts::Actor::user().id {
                let stored = run
                    .user
                    .request("message.get", json!({"id":message.id}))
                    .await
                    .map_err(|err| format!("durable read-back unavailable: {err}"))?;
                if stored["status"] != "delivered" || stored["body"] != message.body {
                    return Err(format!(
                        "delivery event disagrees with durable record: {stored}"
                    ));
                }
                return Ok(stored);
            }
        }
    })
    .await;
    let error = match observed {
        Ok(Ok(message)) => {
            eprintln!(
                "{failure}: {} observed and read back after {:?}",
                decision["id"],
                started.elapsed()
            );
            return Ok(message);
        }
        Ok(Err(err)) => err,
        Err(err) => format!("notice observation expired: {err}"),
    };
    // Diagnostics get their own finite bound, including an unresponsive Daemon.
    let mailbox = match tokio::time::timeout(
        Duration::from_secs(2),
        run.user.request("message.list", json!({})),
    )
    .await
    {
        Ok(Ok(messages)) => {
            let relevant: Vec<_> = messages
                .as_array()
                .unwrap()
                .iter()
                .filter(|m| m["body"].as_str().unwrap().contains(&marker))
                .collect();
            serde_json::to_string(&relevant).unwrap()
        }
        Ok(Err(err)) => format!("unavailable: {err}"),
        Err(err) => format!("unavailable: diagnostic read expired: {err}"),
    };
    Err(format!(
        "{failure}: Decision {}; elapsed {:?}; {error}; last stage {stage}; stored Messages {mailbox}",
        decision["id"],
        started.elapsed()
    ))
}

#[tokio::test]
async fn b37_a_failed_decision_note_is_retained_or_visible_to_the_user() {
    // B50(e): a note never reaches the Terminal, so a Terminal refusal cannot fail one; the failure
    // of its inbox is a hold (B51), not a drop.
    for failure in ["send", "full"] {
        let mut run = Run::start().await;
        let door = run
            .user
            .request("rail.createWorkstream", json!({"name":"supervisor"}))
            .await
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        run.user
            .request("agent.stop", json!({"id":run.child}))
            .await
            .unwrap();
        run.door = Some(door.clone());
        run.spawn().await;
        let mut events = notice_events(&run).await;
        let mut monitor = notice_events(&run).await;
        if failure == "terminal" {
            std::fs::write(run.dir.path().join("decision-notice-fixture"), "").unwrap();
        }
        run.user
            .request("rail.startDoor", json!({"id":door}))
            .await
            .unwrap();
        if failure != "terminal" {
            run.user
                .request("takeover.begin", json!({"agent":door}))
                .await
                .unwrap();
        }
        if failure == "full" {
            // Setup may already have queued a Digest. Fill the actual remaining capacity.
            for _ in 0..=32 {
                if let Err(err) = run
                    .user
                    .request(
                        "message.send",
                        json!({"to":door,"kind":"note","body":"queued"}),
                    )
                    .await
                {
                    assert_eq!(err.code, rpc::code::CONFLICT);
                    assert!(err.message.contains("32 pending or held"), "{err}");
                    break;
                }
            }
        }
        if failure == "send" {
            run.user
                .request("agent.stop", json!({"id":door}))
                .await
                .unwrap();
        }
        // A background Steer must submit before refusal of the Decision note can be isolated.
        let occupying = if failure == "terminal" {
            tokio::time::timeout(BOUND, async {
                loop {
                    if matches!(monitor.next_event().await.unwrap().data,
                        EventData::MessageDropped(ref m) | EventData::MessageDelivered(ref m)
                        if m.to == door && m.body.starts_with("[digest]"))
                    {
                        break;
                    }
                }
            })
            .await
            .unwrap();
            let mut selector = Client::connect(&run.daemon.socket).await.unwrap();
            let mut selected = selector.take_output().unwrap();
            let terminals = run.user.request("terminal.list", ()).await.unwrap();
            for terminal in terminals
                .as_array()
                .unwrap()
                .iter()
                .filter(|terminal| terminal["running"] == true)
            {
                selector
                    .request("terminal.output", json!({"id":terminal["id"]}))
                    .await
                    .unwrap();
            }
            std::fs::write(run.dir.path().join("background-gate"), "").unwrap();
            let sender = Arc::clone(&run.user);
            let receiver = door.clone();
            let task = tokio::spawn(async move {
                sender
                    .request(
                        "message.send",
                        json!({"to":receiver,"kind":"note","body":"occupying steer"}),
                    )
                    .await
            });
            tokio::time::timeout(BOUND, async {
                let mut output = String::new();
                // The paste ends with the B39 id line, then the bracketed-paste end and the Enter.
                let expected: String = b"]\x1b[201~\r"
                    .iter()
                    .map(|byte| format!("BYTE:{byte:02x}\n"))
                    .collect();
                loop {
                    if let rpc::OutputFrame::Bytes(out) = selected.recv().await.unwrap() {
                        output.push_str(&String::from_utf8_lossy(
                            &STANDARD.decode(out.data).unwrap(),
                        ));
                        if output.contains(&expected) {
                            break;
                        }
                    }
                }
            })
            .await
            .unwrap();
            Some(task)
        } else {
            None
        };
        let (decision, waiting) = run.open("permission").await;
        if failure == "terminal" {
            tokio::time::timeout(BOUND, async {
                loop {
                    if matches!(monitor.next_event().await.unwrap().data, EventData::MessageSent(ref m)
                        if m.body.contains(&format!("[decision {}]", decision["id"].as_str().unwrap()))) { break; }
                }
            }).await.unwrap();
            std::fs::remove_file(run.dir.path().join("background-gate")).unwrap();
        }
        let notice = failed_notice(
            &run,
            &mut events,
            &decision,
            failure,
            None,
            Duration::from_secs(40),
        )
        .await
        .unwrap_or_else(|err| panic!("{err}"));
        if let Some(task) = occupying {
            let background = task.await.unwrap().unwrap();
            assert_eq!(
                run.user
                    .request("message.get", json!({"id":background["id"]}))
                    .await
                    .unwrap()["status"],
                "delivered"
            );
        }
        assert!(notice["body"].as_str().unwrap().contains("delivery failed"));
        assert_eq!(notice["status"], "delivered");
        if failure == "full" {
            assert!(
                notice["body"]
                    .as_str()
                    .unwrap()
                    .contains("32 pending or held"),
                "{notice}"
            );
        }
        if failure == "terminal" {
            let refused = run
                .user
                .request("message.get", json!({"id":notice["replyTo"]}))
                .await
                .unwrap();
            assert_eq!(refused["status"], "dropped");
            assert_eq!(refused["reason"], "not accepted");
            assert_eq!(refused["to"], door);
        }
        assert_eq!(
            run.user
                .request("message.get", json!({"id":notice["id"]}))
                .await
                .unwrap(),
            notice
        );
        assert_eq!(
            run.user.request("decision.list", ()).await.unwrap()[0],
            decision
        );
        run.user_answer(&decision, "permission").await;
        let outcome = waiting.await.unwrap().unwrap();
        assert!(
            outcome["output"].as_str().unwrap().contains("allow"),
            "{outcome}"
        );
        run.finish().await;
    }
}

#[tokio::test]
async fn b38_a_missing_notice_fails_with_bounded_delivery_evidence() {
    for stopped in [false, true] {
        let mut run = Run::start().await;
        let door = run
            .user
            .request("rail.createWorkstream", json!({"name":"supervisor"}))
            .await
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        run.user
            .request("agent.stop", json!({"id":run.child}))
            .await
            .unwrap();
        run.door = Some(door.clone());
        run.spawn().await;
        std::fs::write(run.dir.path().join("decision-notice-fixture"), "").unwrap();
        let mut events = notice_events(&run).await;
        let mut monitor = notice_events(&run).await;
        run.user
            .request("rail.startDoor", json!({"id":door}))
            .await
            .unwrap();
        tokio::time::timeout(BOUND, async {
            loop {
                if matches!(monitor.next_event().await.unwrap().data,
                    EventData::MessageDelivered(ref m) | EventData::MessageDropped(ref m)
                    if m.to == door && m.body.starts_with("[digest]"))
                {
                    break;
                }
            }
        })
        .await
        .unwrap();
        // The real Terminal writer acknowledges arrival but cannot complete or refuse yet.
        let gate = run.dir.path().join("input-gate");
        std::fs::write(&gate, "pause").unwrap();
        let (decision, waiting) = run.open("permission").await;
        let marker = format!("[decision {}]", decision["id"].as_str().unwrap());
        let sent = tokio::time::timeout(BOUND, async {
            loop {
                if let EventData::MessageSent(m) = events.next_event().await.unwrap().data
                    && m.body.contains(&marker)
                {
                    break m;
                }
            }
        })
        .await
        .unwrap();
        let signal = |name: &str| {
            assert!(
                std::process::Command::new("kill")
                    .args([name, &run.daemon.child.id().to_string()])
                    .status()
                    .unwrap()
                    .success()
            );
        };
        if stopped {
            signal("-STOP");
        }
        let observed = tokio::time::timeout(
            Duration::from_secs(8),
            failed_notice(
                &run,
                &mut events,
                &decision,
                "terminal",
                Some(&sent),
                Duration::from_millis(300),
            ),
        )
        .await;
        if stopped {
            signal("-CONT");
        }
        std::fs::remove_file(gate).unwrap();
        let error = observed
            .expect("observation and diagnostic reads are bounded")
            .unwrap_err();
        assert!(error.contains("terminal: Decision"), "{error}");
        assert!(error.contains(decision["id"].as_str().unwrap()), "{error}");
        assert!(
            error.contains("elapsed") && error.contains("last stage message.sent"),
            "{error}"
        );
        assert!(error.contains(&format!("\"id\":{}", sent.id)), "{error}");
        assert!(
            error.contains("\"status\":\"pending\"") && error.contains("\"reason\":null"),
            "{error}"
        );
        if stopped {
            assert!(
                error.contains("stored Messages unavailable: diagnostic read expired"),
                "{error}"
            );
        } else {
            assert!(!error.contains("unavailable"), "{error}");
            assert!(error.contains("stored Messages [{"), "{error}");
        }
        run.user_answer(&decision, "permission").await;
        assert!(
            waiting.await.unwrap().unwrap()["output"]
                .as_str()
                .unwrap()
                .contains("allow")
        );
        run.finish().await;
    }
}

#[tokio::test]
async fn b37_decision_note_exclusions_still_hold() {
    for exclusion in ["takeover", "unanswerable", "no Door"] {
        let mut run = Run::start().await;
        let door = run
            .user
            .request("rail.createWorkstream", json!({"name":"supervisor"}))
            .await
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        if exclusion != "no Door" {
            run.user
                .request("agent.stop", json!({"id":run.child}))
                .await
                .unwrap();
            run.door = Some(door.clone());
            run.spawn().await;
        }
        if exclusion == "takeover" {
            run.user
                .request("takeover.begin", json!({"agent":run.child}))
                .await
                .unwrap();
        }
        let kind = if exclusion == "unanswerable" {
            "native"
        } else {
            "permission"
        };
        let (excluded, waiting) = run.open(kind).await;
        run.user_answer(&excluded, kind).await;
        waiting.await.unwrap().unwrap();
        // A later eligible Decision's Message acknowledges the serial note consumer has
        // passed the excluded Decision; no duration stands in for that ordering boundary.
        let excluded_child = run.child.clone();
        run.door = Some(door.clone());
        run.spawn().await;
        let (witness, waiting) = run.open("permission").await;
        let messages = tokio::time::timeout(BOUND, async {
            loop {
                let messages = run.user.request("message.list", json!({})).await.unwrap();
                if messages.as_array().unwrap().iter().any(|m| {
                    m["body"]
                        .as_str()
                        .unwrap()
                        .starts_with(&format!("[decision {}]", witness["id"].as_str().unwrap()))
                }) {
                    break messages;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(
            !messages.as_array().unwrap().iter().any(|m| m["body"]
                .as_str()
                .unwrap()
                .starts_with(&format!("[decision {}]", excluded["id"].as_str().unwrap()))),
            "{exclusion}: {messages}"
        );
        run.user_answer(&witness, "permission").await;
        waiting.await.unwrap().unwrap();
        run.user
            .request("agent.stop", json!({"id":excluded_child}))
            .await
            .unwrap();
        run.finish().await;
    }
}

async fn door_terminal() -> (Run, Arc<Client>) {
    let (mut run, caller) = order_child().await;
    // Agent panes are ownerless (M14); a child-owned Run is in the Door's Reach.
    let child = Client::connect(&run.daemon.socket).await.unwrap();
    child
        .request(
            "daemon.identify",
            json!({"actor":{"kind":"agent","id":run.child}}),
        )
        .await
        .unwrap();
    let opened = child.request("terminal.spawn", json!({
        "cwd":run.dir.path(), "cols":80,"rows":24,"env":{},
        "command":["python3","-u","-c", "import os, tty\ntty.setraw(0)\nprint('READY')\nwhile True:\n b = os.read(0, 1)\n if not b: break\n print('BARRIER' if b == b'\\x00' else 'BYTE:' + b.hex())"]
    })).await.unwrap();
    run.terminal = opened["id"].clone();
    tokio::time::timeout(BOUND, async {
        while !run.screen().await.contains("READY") {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    (run, caller)
}

#[tokio::test]
async fn b6_ordinary_door_write_waits_for_earlier_input_and_its_physical_effect() {
    let (run, caller) = door_terminal().await;
    let gate = run.dir.path().join("input-gate");
    std::fs::write(&gate, "pause").unwrap();
    // Direct user input still acknowledges enqueue while its physical write is held.
    run.user
        .request(
            "terminal.write",
            json!({"id":run.terminal,"data":STANDARD.encode(b"user")}),
        )
        .await
        .unwrap();
    tokio::time::timeout(BOUND, async {
        while !gate.with_extension("arrived").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let terminal = run.terminal.clone();
    let mut completion = tokio::spawn(async move {
        caller
            .request(
                "terminal.write",
                json!({"id":terminal,"data":STANDARD.encode(b"door")}),
            )
            .await
    });
    // This window observes absence of completion, rather than assuming task scheduling.
    let early = tokio::time::timeout(Duration::from_millis(200), &mut completion).await;
    std::fs::remove_file(gate).unwrap();
    assert!(
        early.is_err(),
        "Door completed while its bytes were queued: {early:?}"
    );
    completion.await.unwrap().unwrap();
    assert_eq!(recorded_bytes(&run).await, b"userdoor");
    run.finish().await;
}

#[tokio::test]
async fn b6_ordinary_door_write_refuses_a_full_queue_before_any_bytes() {
    let (run, caller) = door_terminal().await;
    let gate = run.dir.path().join("input-gate");
    std::fs::write(&gate, "pause").unwrap();
    run.user
        .request(
            "terminal.write",
            json!({"id":run.terminal,"data":STANDARD.encode(b"first")}),
        )
        .await
        .unwrap();
    tokio::time::timeout(BOUND, async {
        while !gate.with_extension("arrived").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut accepted = 0;
    loop {
        match run
            .user
            .request(
                "terminal.write",
                json!({"id":run.terminal,"data":STANDARD.encode(b"x")}),
            )
            .await
        {
            Ok(_) => accepted += 1,
            Err(error) => {
                assert_eq!(error.code, rpc::code::CONFLICT);
                break;
            }
        }
    }
    let error = caller
        .request(
            "terminal.write",
            json!({"id":run.terminal,"data":STANDARD.encode(b"refused")}),
        )
        .await
        .unwrap_err();
    std::fs::remove_file(gate).unwrap();
    assert_eq!(error.code, rpc::code::CONFLICT);
    tokio::time::timeout(BOUND, async {
        while run.screen().await.matches("BYTE:78").count() < accepted {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        recorded_bytes(&run).await,
        [b"first".as_slice(), &vec![b'x'; accepted]].concat()
    );
    run.finish().await;
}

async fn uncertain_terminal_write(failure: &str, expected: &[u8]) {
    let (run, caller) = door_terminal().await;
    let gate = run.dir.path().join("input-gate");
    std::fs::write(&gate, failure).unwrap();
    let error = caller
        .request(
            "terminal.write",
            json!({"id":run.terminal,"data":STANDARD.encode(b"uncertain")}),
        )
        .await
        .unwrap_err();
    std::fs::remove_file(gate).unwrap();
    assert_eq!(
        error.code,
        rpc::code::UNKNOWN_OUTCOME,
        "{failure}: {error:?}"
    );
    assert!(error.message.contains("do not retry"), "{error:?}");
    // A later completed Door write and child barrier account for all earlier effects.
    caller
        .request(
            "terminal.write",
            json!({"id":run.terminal,"data":STANDARD.encode(b"later")}),
        )
        .await
        .unwrap();
    assert_eq!(
        recorded_bytes(&run).await,
        [expected, b"later"].concat(),
        "{failure}"
    );
    run.finish().await;
}

#[tokio::test]
async fn b6_ordinary_door_partial_write_is_unknown_without_retry() {
    uncertain_terminal_write("partial", b"u").await;
}

#[tokio::test]
async fn b6_ordinary_door_flush_failure_is_unknown_without_retry() {
    uncertain_terminal_write("flush", b"uncertain").await;
}

#[tokio::test]
async fn b6_ordinary_door_write_lost_input_thread_result_stays_unknown() {
    let (run, caller) = door_terminal().await;
    let gate = run.dir.path().join("input-gate");
    std::fs::write(&gate, "end").unwrap();
    let error = caller
        .request(
            "terminal.write",
            json!({"id":run.terminal,"data":STANDARD.encode(b"unknown")}),
        )
        .await
        .unwrap_err();
    std::fs::remove_file(gate).unwrap();
    assert_eq!(error.code, rpc::code::UNKNOWN_OUTCOME);
    assert!(error.message.contains("do not retry"), "{error:?}");
    assert!(error.message.contains("input ended"), "{error:?}");
    // The failed input thread cannot accept or replay bytes through a replacement queue.
    let later = caller
        .request(
            "terminal.write",
            json!({"id":run.terminal,"data":STANDARD.encode(b"later")}),
        )
        .await
        .unwrap_err();
    assert_ne!(later.code, rpc::code::UNKNOWN_OUTCOME);
    // portable-pty's writer Drop may send newline/EOF; neither is a byte from
    // the lost call or the refused later call.
    let output = run.screen().await;
    assert!(
        !output.contains("BYTE:75"),
        "unknown bytes replayed: {output}"
    );
    assert!(!output.contains("BYTE:6c"), "later bytes written: {output}");
    run.finish().await;
}

#[tokio::test]
async fn b6_interrupt_completion_accounts_for_a_blocked_escape_before_takeover() {
    let (run, caller) = order_child().await;
    idle(&run).await;
    let gate = run.dir.path().join("order-effect-gate");
    std::fs::write(&gate, "pause").unwrap();
    let order = order_result(&run, Arc::clone(&caller)).await;
    tokio::time::timeout(BOUND, async {
        while !gate.with_extension("arrived").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    run.user.request("agent.signal", json!({"id":run.child,"attempt":run.attempt,"payload":{"hook_event_name":"UserPromptSubmit","prompt":"working"}})).await.unwrap();
    let child = run.child.clone();
    let mut interrupt =
        tokio::spawn(async move { caller.request("agent.interrupt", json!({"id":child})).await });
    // INTERRUPT_BOUND is the acknowledgement timer being tested. No completion may
    // escape that timer while the input thread still has an Escape to write.
    let early = tokio::time::timeout(
        agents::INTERRUPT_BOUND + Duration::from_millis(200),
        &mut interrupt,
    )
    .await;
    std::fs::remove_file(gate).unwrap();
    assert!(
        early.is_err(),
        "Interrupt completed with its Escape still queued: {early:?}"
    );
    let error = interrupt.await.unwrap().unwrap_err();
    assert_eq!(error.code, rpc::code::NOT_ACKED);
    assert!(error.message.contains("outcome uncertain"), "{error:?}");
    assert!(error.message.contains("do not retry"), "{error:?}");
    let before = recorded_bytes(&run).await;
    assert_eq!(before.iter().filter(|byte| **byte == 0x1b).count(), 3);
    assert_eq!(before.last(), Some(&0x1b));
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    run.user
        .request(
            "terminal.write",
            json!({"id":run.terminal,"data":STANDARD.encode(b"later")}),
        )
        .await
        .unwrap();
    assert_eq!(
        recorded_bytes(&run).await,
        [before, b"later".to_vec()].concat()
    );
    let _ = order.await.unwrap();
    run.finish().await;
}

#[tokio::test]
async fn b6_a_full_terminal_refuses_interrupt_before_bytes() {
    let (run, caller) = order_child().await;
    idle(&run).await;
    let gate = run.dir.path().join("order-effect-gate");
    std::fs::write(&gate, "pause").unwrap();
    let order = order_result(&run, Arc::clone(&caller)).await;
    tokio::time::timeout(BOUND, async {
        while !gate.with_extension("arrived").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    run.user.request("agent.signal", json!({"id":run.child,"attempt":run.attempt,"payload":{"hook_event_name":"UserPromptSubmit","prompt":"working"}})).await.unwrap();
    let observer = Client::connect(&run.daemon.socket).await.unwrap();
    observer
        .request(
            "daemon.identify",
            json!({"actor":{"kind":"ext","id":"b6-observer"}}),
        )
        .await
        .unwrap();
    let mut queued = 0;
    loop {
        match observer
            .request(
                "terminal.write",
                json!({"id":run.terminal,"data":STANDARD.encode(b"x")}),
            )
            .await
        {
            Ok(_) => queued += 1,
            Err(error) => {
                assert_eq!(error.code, rpc::code::CONFLICT);
                break;
            }
        }
    }
    let error = caller
        .request("agent.interrupt", json!({"id":run.child}))
        .await
        .unwrap_err();
    std::fs::remove_file(gate).unwrap();
    assert_eq!(error.code, rpc::code::CONFLICT);
    assert!(error.message.contains("no bytes written"), "{error:?}");
    let expected = [
        agents::claude_code::LEAD_IN.as_bytes(),
        b"\x1b[200~first order\x1b[201~\r",
        &vec![b'x'; queued],
    ]
    .concat();
    tokio::time::timeout(BOUND, async {
        while run.screen().await.matches("BYTE:78").count() < queued {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(recorded_bytes(&run).await, expected);
    let _ = order.await.unwrap();
    run.finish().await;
}

mod cleanup_completion {
    // Resume completion includes the failed Attempt's physical Terminal cleanup.
    use super::rupd_harness;

    use rpc::Client;
    use serde_json::{Value, json};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;

    const BOUND: Duration = Duration::from_secs(15);
    const CONVERSATION: &str = "11111111-1111-4111-8111-111111111111";

    struct Run {
        _daemon: rupd_harness::Rupd,
        dir: tempfile::TempDir,
        _data: tempfile::TempDir,
        user: Arc<Client>,
        child: Value,
        db: rusqlite::Connection,
    }

    async fn arrived(path: &Path) {
        tokio::time::timeout(BOUND, async {
            while !path.is_file() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("observer never reached {}", path.display()));
    }

    impl Run {
        async fn start() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let data = tempfile::tempdir().unwrap();
            let fake = dir.path().join("claude");
            std::fs::write(&fake, r#"#!/usr/bin/env python3
import json, os, signal, subprocess, sys, time, tty
if sys.argv[1:3] == ['plugin', 'list']:
    print('[]'); sys.exit(0)
if sys.argv[1:3] == ['plugin', 'test']:
    print('Q11 runtime hook closure: 1 pass'); sys.exit(0)
tty.setraw(0)
if '--resume' not in sys.argv:
    settings = json.load(open(sys.argv[sys.argv.index('--settings')+1]))
    payload = json.dumps({'hook_event_name':'SessionStart','source':'startup','session_id':'11111111-1111-4111-8111-111111111111'})
    for group in settings['hooks']['SessionStart']:
        for hook in group['hooks']:
            subprocess.run(hook['command'], shell=True, input=payload, text=True, check=True)
    print('SAVED', flush=True)
else:
    print('RESUMED', flush=True)
while True: time.sleep(60)
"#).unwrap();
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
            let rup = Path::new(env!("CARGO_BIN_EXE_rupd")).with_file_name("rup");
            assert!(rup.is_file(), "cargo build -p rup first");
            let daemon = rupd_harness::start(
                Path::new(env!("CARGO_BIN_EXE_rupd")),
                dir.path(),
                true,
                |command| {
                    command
                        .env("ROUNDUP_DATA_DIR", data.path())
                        .env("ROUNDUP_CLAUDE_BIN", &fake)
                        .env("ROUNDUP_RUP_BIN", rup)
                        .env("CLAUDE_CONFIG_DIR", dir.path().join("config"))
                        .env("RUPD_RESUME_EFFECT_GATE", dir.path().join("resume"))
                        .env("RUPD_TERMINAL_EXIT_GATE", dir.path().join("hold-exit"))
                        .stderr(std::process::Stdio::null());
                },
            );
            let user = Arc::new(Client::connect(&daemon.socket).await.unwrap());
            let node = user
                .request("agent.spawn", json!({"name":"child","cwd":dir.path()}))
                .await
                .unwrap();
            tokio::time::timeout(BOUND, async {
                loop {
                    let screen = user
                        .request("terminal.read", json!({"id":node["terminal_id"],"after":0}))
                        .await
                        .unwrap();
                    if screen["text"].as_str().unwrap().contains("SAVED") {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            user.request("agent.stop", json!({"id":node["id"]}))
                .await
                .unwrap();
            let folder = rupd::data::project_data(data.path(), dir.path()).unwrap();
            let db = rusqlite::Connection::open(folder.join("project.db")).unwrap();
            Self {
                _daemon: daemon,
                dir,
                _data: data,
                user,
                child: node["id"].clone(),
                db,
            }
        }

        fn gate(&self, phase: &str) -> PathBuf {
            self.dir.path().join(format!("resume.{phase}"))
        }

        async fn ack(&self) -> Result<Value, rpc::RpcError> {
            self.user.request("agent.signal", json!({"id":self.child,"attempt":"2","payload":{"hook_event_name":"SessionStart","source":"resume","session_id":CONVERSATION}})).await
        }

        async fn node(&self) -> Value {
            let tree = self.user.request("rail.tree", ()).await.unwrap();
            tree.as_array()
                .unwrap()
                .iter()
                .find(|n| n["id"] == self.child)
                .unwrap()
                .clone()
        }

        async fn terminal_running(&self, terminal: &str) -> bool {
            let terminals = self.user.request("terminal.list", ()).await.unwrap();
            terminals
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["id"] == terminal && t["running"] == true)
        }
    }

    impl Drop for Run {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.dir.path().join("hold-exit"));
            let _ = std::fs::remove_file(self.gate("cleanup"));
            let _ = std::fs::remove_file(self.gate("claim"));
        }
    }

    #[tokio::test]
    async fn b6_resume_completion_waits_for_both_promotion_failure_cleanups() {
        for column in ["terminal_id", "conversation_id"] {
            for early_ack in [false, true] {
                let run = Run::start().await;
                run.db.execute_batch(&format!("CREATE TRIGGER refuse_resume BEFORE UPDATE OF {column} ON nodes WHEN NEW.{column} IS NOT NULL BEGIN SELECT RAISE(ABORT, 'resume {column} fault'); END;")).unwrap();
                std::fs::write(run.gate("cleanup"), "pause").unwrap();
                if early_ack {
                    std::fs::write(run.gate("claim"), "pause").unwrap();
                }
                let caller = Arc::clone(&run.user);
                let child = run.child.clone();
                let mut resume = tokio::spawn(async move {
                    caller.request("agent.resume", json!({"id":child})).await
                });
                arrived(&run.gate(if early_ack {
                    "claim-arrived"
                } else {
                    "claimed-arrived"
                }))
                .await;
                let ack = run.ack().await;
                if early_ack {
                    ack.unwrap();
                    std::fs::remove_file(run.gate("claim")).unwrap();
                } else {
                    assert!(ack.unwrap_err().message.contains("fault"));
                }
                arrived(&run.gate("cleanup-arrived")).await;
                let terminal = std::fs::read_to_string(run.gate("cleanup-arrived")).unwrap();
                assert!(
                    run.terminal_running(&terminal).await,
                    "cleanup marker named {terminal:?}"
                );
                let retry = run
                    .user
                    .request("agent.resume", json!({"id":run.child}))
                    .await
                    .unwrap_err();
                assert!(retry.message.contains(&terminal), "{retry:?}");
                assert_eq!(
                    run.node().await["attempt"],
                    "2",
                    "retry allocated an Attempt during cleanup"
                );
                // The acknowledgement timer must not escape cleanup, even when it expires.
                let early = tokio::time::timeout(
                    agents::RESUME_ACK_BOUND + Duration::from_millis(100),
                    &mut resume,
                )
                .await;
                std::fs::remove_file(run.gate("cleanup")).unwrap();
                assert!(
                    early.is_err(),
                    "Resume completed before its cleanup: {early:?}"
                );
                let error = tokio::time::timeout(BOUND, resume)
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap_err();
                assert!(
                    error.message.contains(&format!("resume {column} fault")),
                    "{error:?}"
                );
                assert!(
                    !run.terminal_running(&terminal).await,
                    "cleanup reported before exit"
                );
                // A following Resume gets a new Terminal. No task from the failed one can kill it.
                run.db.execute_batch("DROP TRIGGER refuse_resume;").unwrap();
                let caller = Arc::clone(&run.user);
                let child = run.child.clone();
                let next = tokio::spawn(async move {
                    caller.request("agent.resume", json!({"id":child})).await
                });
                tokio::time::timeout(BOUND, async {
                    loop {
                        let tree = run.user.request("rail.tree", ()).await.unwrap();
                        if tree
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|n| n["id"] == run.child && n["attempt"] == "3")
                        {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                run.user.request("agent.signal", json!({"id":run.child,"attempt":"3","payload":{"hook_event_name":"SessionStart","source":"resume","session_id":CONVERSATION}})).await.unwrap();
                let node = tokio::time::timeout(BOUND, next)
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();
                run.user
                    .request("takeover.begin", json!({"agent":run.child}))
                    .await
                    .unwrap();
                assert!(
                    run.terminal_running(node["terminal_id"].as_str().unwrap())
                        .await
                );
                run.user
                    .request("agent.stop", json!({"id":run.child}))
                    .await
                    .unwrap();
            }
        }
    }

    #[tokio::test]
    async fn b6_cleanup_refusal_preserves_the_resume_failure_and_blocks_a_blind_retry() {
        failed_cleanup(false).await;
    }

    #[tokio::test]
    async fn b6_unconfirmed_terminal_exit_preserves_the_resume_failure_and_blocks_a_blind_retry() {
        failed_cleanup(true).await;
    }

    async fn failed_cleanup(uncertain: bool) {
        let run = Run::start().await;
        run.db.execute_batch("CREATE TRIGGER refuse_resume BEFORE UPDATE OF conversation_id ON nodes BEGIN SELECT RAISE(ABORT, 'resume save fault'); END;").unwrap();
        if uncertain {
            std::fs::write(run.dir.path().join("hold-exit"), "hold").unwrap();
        } else {
            std::fs::write(run.gate("cleanup"), "refuse").unwrap();
        }
        let caller = Arc::clone(&run.user);
        let child = run.child.clone();
        let resume =
            tokio::spawn(async move { caller.request("agent.resume", json!({"id":child})).await });
        arrived(&run.gate("claimed-arrived")).await;
        assert!(
            run.ack()
                .await
                .unwrap_err()
                .message
                .contains("resume save fault")
        );
        let error = tokio::time::timeout(BOUND, resume)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        let terminal = std::fs::read_to_string(run.gate("cleanup-arrived")).unwrap();
        assert_eq!(error.code, rpc::code::UNKNOWN_OUTCOME, "{error:?}");
        assert!(error.message.contains("resume save fault"), "{error:?}");
        assert!(error.message.contains(&terminal), "{error:?}");
        assert!(error.message.contains("do not retry Resume"), "{error:?}");
        assert!(
            error.message.contains(if uncertain {
                "exit not confirmed"
            } else {
                "refused before kill"
            }),
            "{error:?}"
        );
        assert!(run.terminal_running(&terminal).await);
        let retry = run
            .user
            .request("agent.resume", json!({"id":run.child}))
            .await
            .unwrap_err();
        assert_eq!(retry, error, "a retry lost the original outcome");
        let node = run.node().await;
        assert_eq!(node["attempt"], "2");
        assert_eq!(node["terminal_id"], terminal);
        assert_eq!(node["can_resume"], false);
        // Only explicit resolution of the remaining Terminal makes another Resume safe.
        if uncertain {
            let stop = run
                .user
                .request("agent.stop", json!({"id":run.child}))
                .await
                .unwrap_err();
            assert_eq!(stop.code, rpc::code::UNKNOWN_OUTCOME, "{stop:?}");
            assert_eq!(
                run.node().await["can_resume"],
                false,
                "an unconfirmed stop authorized Resume"
            );
            std::fs::remove_file(run.dir.path().join("hold-exit")).unwrap();
            tokio::time::timeout(BOUND, async {
                while run.terminal_running(&terminal).await {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        } else {
            std::fs::remove_file(run.gate("cleanup")).unwrap();
        }
        run.user
            .request("agent.stop", json!({"id":run.child}))
            .await
            .unwrap();
        assert!(!run.terminal_running(&terminal).await);
        assert_eq!(run.node().await["can_resume"], true);
    }

    #[tokio::test]
    async fn b6_ack_timeout_waits_for_rollback_cleanup_and_accounts_for_refusal() {
        let run = Run::start().await;
        std::fs::write(run.gate("cleanup"), "pause").unwrap();
        let caller = Arc::clone(&run.user);
        let child = run.child.clone();
        let mut resume =
            tokio::spawn(async move { caller.request("agent.resume", json!({"id":child})).await });
        arrived(&run.gate("cleanup-arrived")).await;
        let terminal = std::fs::read_to_string(run.gate("cleanup-arrived")).unwrap();
        let early = tokio::time::timeout(Duration::from_millis(100), &mut resume).await;
        std::fs::write(run.gate("cleanup"), "refuse").unwrap();
        assert!(
            early.is_err(),
            "timeout escaped rollback cleanup: {early:?}"
        );
        let error = tokio::time::timeout(BOUND, resume)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert_eq!(error.code, rpc::code::UNKNOWN_OUTCOME);
        assert!(
            error.message.contains("no acknowledgement within 10s"),
            "{error:?}"
        );
        assert!(error.message.contains("refused before kill"), "{error:?}");
        assert!(error.message.contains(&terminal), "{error:?}");
        assert!(run.terminal_running(&terminal).await);
        run.user
            .request("agent.stop", json!({"id":run.child}))
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn b6_takeover_acquisition_waits_for_a_door_spawned_first_prompt() {
    let (mut run, caller) = order_child().await;
    run.user
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    let gate = run.dir.path().join("input-gate");
    std::fs::write(&gate, "pause").unwrap();
    let node = caller
        .request(
            "agent.spawn",
            json!({"name":"first prompt child","cwd":run.dir.path(),"prompt":"first prompt","order":{"kind":"work","ask":"first prompt","limits":[]}}),
        )
        .await
        .unwrap();
    run.child = node["id"].as_str().unwrap().to_owned();
    run.terminal = node["terminal_id"].clone();
    run.attempt = node["attempt"].clone();
    tokio::time::timeout(BOUND, async {
        while !gate.with_extension("arrived").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let user = Arc::clone(&run.user);
    let child = run.child.clone();
    let mut begin =
        tokio::spawn(async move { user.request("takeover.begin", json!({"agent":child})).await });
    // The physical input observer acknowledges the first Steer before this absence window.
    let early = tokio::time::timeout(Duration::from_millis(200), &mut begin).await;
    std::fs::remove_file(&gate).unwrap();
    assert!(
        early.is_err(),
        "Takeover acquired before the first prompt effect: {early:?}"
    );
    tokio::time::timeout(BOUND, begin)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let held = caller
        .request(
            "message.send",
            json!({"to":run.child,"kind":"note","body":"after acquisition"}),
        )
        .await
        .unwrap();
    assert_eq!(held["status"], "held");
    assert_eq!(held["reason"], "takeover");
    run.finish().await;
}

async fn pause_door_write(
    run: &Run,
    caller: Arc<Client>,
) -> tokio::task::JoinHandle<Result<Value, RpcError>> {
    let gate = run.dir.path().join("input-gate");
    std::fs::write(&gate, "pause").unwrap();
    let terminal = run.terminal.clone();
    let write = tokio::spawn(async move {
        caller
            .request(
                "terminal.write",
                json!({"id":terminal,"data":STANDARD.encode(b"once")}),
            )
            .await
    });
    tokio::time::timeout(BOUND, async {
        while !gate.with_extension("arrived").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    write
}

async fn release_door_write(run: &Run, write: tokio::task::JoinHandle<Result<Value, RpcError>>) {
    std::fs::remove_file(run.dir.path().join("input-gate")).unwrap();
    tokio::time::timeout(BOUND, write)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn b6_begin_racing_a_door_write_waits_for_the_child_effect() {
    let (run, caller) = door_terminal().await;
    let write = pause_door_write(&run, caller).await;
    let user = Arc::clone(&run.user);
    let child = run.child.clone();
    let mut begin =
        tokio::spawn(async move { user.request("takeover.begin", json!({"agent":child})).await });
    // The physical input observer establishes that the Door call is inside its effect.
    let early = tokio::time::timeout(Duration::from_millis(200), &mut begin).await;
    release_door_write(&run, write).await;
    assert!(
        early.is_err(),
        "Takeover acquired before the Door write: {early:?}"
    );
    tokio::time::timeout(BOUND, begin)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(recorded_bytes(&run).await, b"once");
    run.finish().await;
}

#[tokio::test]
async fn b6_another_childs_takeover_does_not_wait_for_a_door_write() {
    let (run, caller) = door_terminal().await;
    let other = caller
        .request(
            "agent.spawn",
            json!({"name":"other child","cwd":run.dir.path()}),
        )
        .await
        .unwrap();
    tokio::time::timeout(BOUND, async {
        loop {
            let output = run
                .user
                .request(
                    "terminal.read",
                    json!({"id":other["terminal_id"],"after":0}),
                )
                .await
                .unwrap();
            if output["text"].as_str().unwrap().contains("READY") {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let write = pause_door_write(&run, caller).await;
    let begin = tokio::time::timeout(
        Duration::from_secs(1),
        run.user
            .request("takeover.begin", json!({"agent":other["id"]})),
    )
    .await;
    release_door_write(&run, write).await;
    begin
        .expect("another child's Takeover waited for the paused write")
        .unwrap();
    run.user
        .request("agent.stop", json!({"id":other["id"]}))
        .await
        .unwrap();
    run.finish().await;
}

#[tokio::test]
async fn b6_direct_user_input_does_not_wait_for_a_door_write() {
    let (run, caller) = door_terminal().await;
    let write = pause_door_write(&run, caller).await;
    // Direct input acknowledges enqueue even while the Door's physical write is paused.
    let user_write = tokio::time::timeout(
        Duration::from_secs(1),
        run.user.request(
            "terminal.write",
            json!({"id":run.terminal,"data":STANDARD.encode(b"user")}),
        ),
    )
    .await;
    release_door_write(&run, write).await;
    user_write
        .expect("direct user input waited for the Door write")
        .unwrap();
    assert_eq!(recorded_bytes(&run).await, b"onceuser");
    run.finish().await;
}

async fn steering_takeover_race(method: &'static str) {
    let (run, caller) = order_child().await;
    idle(&run).await;
    std::fs::write(run.dir.path().join("order-submit"), "accept").unwrap();
    if method == "agent.interrupt" {
        run.user.request("agent.signal", json!({"id":run.child,"attempt":run.attempt,"payload":{"hook_event_name":"UserPromptSubmit","prompt":"working"}})).await.unwrap();
    }
    let gate = run.dir.path().join("input-gate");
    std::fs::write(&gate, "pause").unwrap();
    let child = run.child.clone();
    let action = tokio::spawn(async move {
        caller.request(method, json!({"id":child,"text":"one prompt","order":{"kind":"work","ask":"one order","limits":[]}})).await
    });
    tokio::time::timeout(BOUND, async {
        while !gate.with_extension("arrived").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let user = Arc::clone(&run.user);
    let child = run.child.clone();
    let mut begin =
        tokio::spawn(async move { user.request("takeover.begin", json!({"agent":child})).await });
    // The physical input observer establishes that the action is inside its effect.
    let early = tokio::time::timeout(Duration::from_millis(200), &mut begin).await;
    std::fs::remove_file(&gate).unwrap();
    let outcome = tokio::time::timeout(BOUND, action).await.unwrap().unwrap();
    assert!(
        early.is_err(),
        "Takeover acquired before {method}'s effect: {early:?}"
    );
    if method == "agent.interrupt" {
        assert_eq!(outcome.unwrap_err().code, rpc::code::NOT_ACKED);
    } else {
        outcome.unwrap();
    }
    tokio::time::timeout(BOUND, begin)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let bytes = recorded_bytes(&run).await;
    match method {
        "agent.prompt" => assert!(bytes.ends_with(b"one prompt\x1b[201~\r")),
        "agent.setOrder" => assert!(bytes.ends_with(b"one order\x1b[201~\r")),
        _ => assert_eq!(bytes, b"\x1b"),
    }
    run.finish().await;
}

#[tokio::test]
async fn b6_begin_racing_a_door_prompt_waits_for_the_child_effect() {
    steering_takeover_race("agent.prompt").await;
}

#[tokio::test]
async fn b6_begin_racing_a_door_order_waits_for_the_child_effect() {
    steering_takeover_race("agent.setOrder").await;
}

#[tokio::test]
async fn b6_begin_racing_a_door_interrupt_waits_for_the_child_effect() {
    steering_takeover_race("agent.interrupt").await;
}

#[tokio::test]
async fn b37_a_stop_failure_door_receives_its_answered_childs_decision_note() {
    let mut run = Run::start().await;
    let door = run
        .user
        .request("rail.createWorkstream", json!({"name":"supervisor"}))
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    run.user
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    run.door = Some(door.clone());
    run.spawn().await;
    std::fs::write(run.dir.path().join("order-submit"), "").unwrap();
    let mut events = notice_events(&run).await;
    run.user
        .request("rail.startDoor", json!({"id":door}))
        .await
        .unwrap();
    let door_node = tokio::time::timeout(BOUND, async {
        loop {
            let tree = run.user.request("rail.tree", ()).await.unwrap();
            let node = tree
                .as_array()
                .unwrap()
                .iter()
                .find(|n| n["id"] == door)
                .unwrap();
            if node["status"]["kind"] == "idle" {
                break node.clone();
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    run.user
        .request("takeover.begin", json!({"agent":door}))
        .await
        .unwrap();
    let (decision, waiting) = run.open("permission").await;
    let marker = format!("[decision {}]", decision["id"].as_str().unwrap());
    let note = tokio::time::timeout(BOUND, async {
        loop {
            if let EventData::MessageSent(m) = events.next_event().await.unwrap().data
                && m.to == door
                && m.body.starts_with(&marker)
            {
                break m;
            }
        }
    })
    .await
    .unwrap();
    run.user.request("agent.signal", json!({"id":door,"attempt":door_node["attempt"],"payload":{"hook_event_name":"StopFailure","error":"server_error"}})).await.unwrap();
    tokio::time::timeout(BOUND, async {
        loop {
            if let EventData::AgentStatus(s) = events.next_event().await.unwrap().data
                && s.id == door
                && s.status.kind == contracts::Kind::Error
            {
                break;
            }
        }
    })
    .await
    .unwrap();
    let caller = Client::connect(&run.daemon.socket).await.unwrap();
    caller
        .request(
            "daemon.identify",
            json!({"actor":{"kind":"agent","id":door}}),
        )
        .await
        .unwrap();
    caller
        .request(
            "decision.answer",
            json!({"id":decision["id"],"answer":"allow","proof":"0".repeat(64)}),
        )
        .await
        .unwrap();
    assert!(waiting.await.unwrap().is_ok());
    run.user
        .request("takeover.end", json!({"agent":door}))
        .await
        .unwrap();
    tokio::time::timeout(BOUND, async {
        let mut delivered = false;
        let mut working = false;
        while !delivered || !working {
            match events.next_event().await.unwrap().data {
                EventData::MessageDelivered(m) if m.id == note.id => delivered = true,
                EventData::AgentStatus(s)
                    if s.id == door && s.status.kind == contracts::Kind::Working =>
                {
                    working = true
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    let stored = run
        .user
        .request("message.get", json!({"id":note.id}))
        .await
        .unwrap();
    assert_eq!(stored["status"], "delivered");
    // B50: the note is one line on the Door's messaging socket, and none of it is typed.
    let inbox =
        std::fs::read_to_string(run.dir.path().join(format!("received-{door}.jsonl"))).unwrap();
    assert_eq!(inbox.matches(&marker).count(), 1, "{inbox}");
    let screen = run
        .user
        .request(
            "terminal.read",
            json!({"id":door_node["terminal_id"],"after":0}),
        )
        .await
        .unwrap()["text"]
        .as_str()
        .unwrap()
        .to_owned();
    let typed: Vec<u8> = screen
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("BYTE:"))
        .map(|hex| u8::from_str_radix(hex.trim(), 16).unwrap())
        .collect();
    assert!(
        !String::from_utf8_lossy(&typed).contains(&marker),
        "{screen}"
    );
    run.finish().await;
}

async fn input_after_error(pending: bool) {
    let run = Run::start().await;
    std::fs::write(run.dir.path().join("order-submit"), "").unwrap();
    let mut events = notice_events(&run).await;
    if pending {
        run.user.request("agent.signal", json!({"id":run.child,"attempt":run.attempt,"payload":{"hook_event_name":"UserPromptSubmit"}})).await.unwrap();
        for body in ["one", "two"] {
            run.user
                .request(
                    "message.send",
                    json!({"to":run.child,"kind":"note","body":body}),
                )
                .await
                .unwrap();
        }
    }
    run.user.request("agent.signal", json!({"id":run.child,"attempt":run.attempt,"payload":{"hook_event_name":"StopFailure","error":"server_error"}})).await.unwrap();
    tokio::time::timeout(BOUND, async {
        loop {
            if let EventData::AgentStatus(s) = events.next_event().await.unwrap().data
                && s.id == run.child
                && s.status.kind == contracts::Kind::Error
            {
                break;
            }
        }
    })
    .await
    .unwrap();
    if !pending {
        run.user
            .request(
                "message.send",
                json!({"to":run.child,"kind":"note","body":"one"}),
            )
            .await
            .unwrap();
    }
    tokio::time::timeout(BOUND, async {
        let mut delivered = 0;
        let mut working = false;
        while delivered < if pending { 2 } else { 1 } || !working {
            match events.next_event().await.unwrap().data {
                EventData::MessageDelivered(m) if m.to == run.child => delivered += 1,
                EventData::AgentStatus(s)
                    if s.id == run.child && s.status.kind == contracts::Kind::Working =>
                {
                    assert_eq!(s.attempt, run.attempt.as_str().unwrap());
                    working = true;
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    let text = String::from_utf8(recorded_bytes(&run).await).unwrap();
    let expected = if pending {
        "[from you, note] one\n\n[from you, note] two"
    } else {
        "[from you, note] one"
    };
    assert_eq!(text.matches(expected).count(), 1, "{text}");
    assert_eq!(text.matches('\r').count(), 1, "one prompt: {text}");
    run.finish().await;
}

#[tokio::test]
async fn b37_a_live_error_accepts_and_delivers_a_new_message() {
    input_after_error(false).await;
}

#[tokio::test]
async fn b37_stop_failure_delivers_pending_messages_in_one_prompt() {
    input_after_error(true).await;
}

#[tokio::test]
async fn b6_begin_racing_a_door_resume_waits_for_the_new_attempt() {
    let (run, caller) = order_child().await;
    run.user
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    let fake = run.dir.path().join("claude");
    let script = std::fs::read_to_string(&fake).unwrap().replace(
        "hook('SessionStart',",
        "if '--resume' in sys.argv:\n    import time\n    open('resume-arrived', 'w').close()\n    while os.path.isfile('resume-gate'): time.sleep(0.005)\nhook('SessionStart',",
    );
    std::fs::write(fake, script).unwrap();
    let gate = run.dir.path().join("resume-gate");
    std::fs::write(&gate, "pause").unwrap();
    let child = run.child.clone();
    let resume =
        tokio::spawn(async move { caller.request("agent.resume", json!({"id":child})).await });
    tokio::time::timeout(BOUND, async {
        while !run.dir.path().join("resume-arrived").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let user = Arc::clone(&run.user);
    let child = run.child.clone();
    let mut begin =
        tokio::spawn(async move { user.request("takeover.begin", json!({"agent":child})).await });
    // The new vendor process acknowledges launch before the absence observation window.
    let early = tokio::time::timeout(Duration::from_millis(200), &mut begin).await;
    std::fs::remove_file(gate).unwrap();
    let resumed = tokio::time::timeout(BOUND, resume)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(
        early.is_err(),
        "Takeover returned before Resume completed: {early:?}"
    );
    tokio::time::timeout(BOUND, begin)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_ne!(resumed["attempt"], run.attempt);
    let message = run
        .user
        .request(
            "message.send",
            json!({"to":run.child,"kind":"note","body":"new Attempt"}),
        )
        .await
        .unwrap();
    assert_eq!(message["status"], "held");
    assert_eq!(message["reason"], "takeover");
    run.finish().await;
}

async fn start_answer_door(run: &Run) -> (Value, Value) {
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    let door = tree
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == run.child)
        .unwrap()["parent"]
        .clone();
    let started = run
        .user
        .request("rail.startDoor", json!({"id":door}))
        .await
        .unwrap();
    let terminal = started["terminal_id"].clone();
    tokio::time::timeout(BOUND, async {
        loop {
            let output = run
                .user
                .request("terminal.read", json!({"id":terminal,"after":0}))
                .await
                .unwrap();
            if output["text"].as_str().unwrap().contains("READY") {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    (door, terminal)
}

async fn answer_takeover_race(kind: &str, guarded: bool) {
    let (run, _) = order_child().await;
    let (door, terminal) = start_answer_door(&run).await;
    let (decision, waiting) = run.open(kind).await;
    let identity = run.capture(&decision).await;
    let gate = run.dir.path().join("effect-gate");
    std::fs::write(&gate, "before").unwrap();
    let mut params =
        json!({"id":decision["id"],"answer":if kind == "permission" { "allow" } else { "Yes" }});
    if guarded {
        params["identity"] = identity;
    }
    std::fs::write(
        run.dir.path().join("peer-request"),
        serde_json::to_vec(&json!({
            "jsonrpc":"2.0", "id":1,"method":"decision.answer",
            "params":params
        }))
        .unwrap(),
    )
    .unwrap();
    // The real Door process calls without identify: connection actor defaults to user.
    run.user
        .request(
            "terminal.write",
            json!({"id":terminal,"data":STANDARD.encode(b"\x02")}),
        )
        .await
        .unwrap();
    run.paused().await;
    let user = Arc::clone(&run.user);
    let child = run.child.clone();
    let mut begin =
        tokio::spawn(async move { user.request("takeover.begin", json!({"agent":child})).await });
    let early = tokio::time::timeout(Duration::from_millis(200), &mut begin).await;
    std::fs::remove_file(&gate).unwrap();
    let result = tokio::time::timeout(BOUND, async {
        loop {
            if let Ok(text) = std::fs::read_to_string(run.dir.path().join("peer-result"))
                && let Ok(value) = serde_json::from_str::<Value>(&text)
            {
                break value;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        early.is_err(),
        "Takeover acquired before the Door answer: {early:?}; {result}"
    );
    assert!(result.get("result").is_some(), "{result}");
    waiting.await.unwrap().unwrap();
    tokio::time::timeout(BOUND, begin)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    run.user
        .request("agent.stop", json!({"id":door}))
        .await
        .unwrap();
    run.finish().await;
}

#[tokio::test]
async fn b6_begin_racing_a_door_answer_uses_the_process_identity() {
    for kind in ["permission", "ask_user"] {
        answer_takeover_race(kind, true).await;
    }
}

#[tokio::test]
async fn b6_begin_racing_a_bare_door_answer_uses_the_process_identity() {
    for kind in ["permission", "ask_user"] {
        answer_takeover_race(kind, false).await;
    }
}

#[tokio::test]
async fn b6_wire_door_decision_calls_wait_for_all_earlier_resizes() {
    for (kind, method) in [
        ("permission", "decision.list"),
        ("ask_user", "decision.list"),
        ("permission", "decision.capture"),
        ("ask_user", "decision.capture"),
        ("permission", "decision.answers"),
        ("ask_user", "decision.answers"),
        ("permission", "decision.answer"),
        ("ask_user", "decision.answer"),
    ] {
        let (run, caller) = door_terminal().await;
        let (door, terminal) = start_answer_door(&run).await;
        let (decision, waiting) = run.open(kind).await;
        let identity = run.capture(&decision).await;
        run.pause();
        let id = run.terminal.clone();
        let first = tokio::spawn(async move {
            caller
                .request("terminal.resize", json!({"id":id,"cols":101,"rows":37}))
                .await
        });
        run.paused().await;
        let mut frames = vec![
            json!({"id":1,"method":"daemon.identify","params":{"actor":{"kind":"agent","id":door}}}),
        ];
        for cols in 102..110 {
            frames.push(json!({"id":cols,"method":"terminal.resize","params":{"id":run.terminal,"cols":cols,"rows":37}}));
        }
        let params = match method {
            "decision.list" => Value::Null,
            "decision.capture" => json!({"id":decision["id"]}),
            "decision.answers" => json!({"agent":run.child}),
            _ => {
                json!({"id":decision["id"],"identity":identity,"answer":if kind == "permission" { "allow" } else { "Yes" }})
            }
        };
        frames.push(json!({"id":110,"method":method,"params":params}));
        frames.push(json!({"id":111,"method":"rail.tree"}));
        std::fs::write(
            run.dir.path().join("peer-request"),
            serde_json::to_vec(&frames).unwrap(),
        )
        .unwrap();
        // The real Door sends the batch, retaining H21's authenticated process identity.
        run.user
            .request(
                "terminal.write",
                json!({"id":terminal,"data":STANDARD.encode(b"\x02")}),
            )
            .await
            .unwrap();
        tokio::time::timeout(BOUND, async {
            while !run.dir.path().join("peer-barrier").is_file() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        run.release();
        let replies: Value = tokio::time::timeout(BOUND, async {
            loop {
                if let Ok(text) = std::fs::read_to_string(run.dir.path().join("peer-result"))
                    && let Ok(value) = serde_json::from_str::<Value>(&text)
                {
                    break value;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        first.await.unwrap().unwrap();
        if method != "decision.answer" {
            run.user_answer(&decision, kind).await;
        }
        waiting.await.unwrap().unwrap();
        for reply in replies.as_array().unwrap() {
            assert!(reply.get("result").is_some(), "{reply}");
        }
        let order: Vec<_> = replies
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_u64().unwrap())
            .collect();
        run.user
            .request("agent.stop", json!({"id":door}))
            .await
            .unwrap();
        run.finish().await;
        assert_eq!(
            order,
            [1, 111, 102, 103, 104, 105, 106, 107, 108, 109, 110],
            "{kind} {method}: Decision call overtook an earlier wire resize"
        );
    }
}

// U468's backend prerequisite; the App observer belongs to #1497.
#[tokio::test]
async fn u468_a_door_answer_remains_visible_with_its_identity() {
    let (mut run, door, door_terminal) = answer_child().await;
    let unrelated = run
        .user
        .request("rail.createWorkstream", json!({"name":"unrelated"}))
        .await
        .unwrap()["id"]
        .clone();
    // Seed an answered request in a different Workstream before the retained ones.
    let original_child = run.child.clone();
    let original_terminal = run.terminal.clone();
    let original_attempt = run.attempt.clone();
    run.door = Some(unrelated.as_str().unwrap().to_owned());
    run.spawn().await;
    let unrelated_child = run.child.clone();
    let (outside_request, outside_waiting) = run.open("permission").await;
    let denied = door_call(
        &run,
        &door_terminal,
        "decision.answer",
        json!({"id":outside_request["id"],"answer":"allow"}),
    )
    .await;
    assert_eq!(denied["error"]["code"], rpc::code::FORBIDDEN);
    assert_eq!(
        run.user.request("decision.list", ()).await.unwrap(),
        json!([outside_request])
    );
    assert_eq!(
        run.user
            .request("decision.answers", json!({"agent":unrelated_child}))
            .await
            .unwrap(),
        json!([])
    );
    run.user_answer(&outside_request, "permission").await;
    outside_waiting.await.unwrap().unwrap();
    run.child = original_child;
    run.terminal = original_terminal;
    run.attempt = original_attempt;
    run.door = Some(door.clone());
    let mut originals = Vec::new();
    for (kind, answer, note) in [
        ("ask_user", "Yes", "Exact note: α\nsecond line"),
        ("permission", "allow", ""),
        ("permission", "deny", ""),
    ] {
        let prompt = if kind == "permission" {
            format!("{}complete tail", "long command α\n".repeat(600))
        } else {
            "Question?\nComplete second line".to_owned()
        };
        let (decision, waiting) = run.open_prompt(kind, &prompt).await;
        let reply = door_call(
            &run,
            &door_terminal,
            "decision.answer",
            json!({"id":decision["id"],"answer":answer,"note":note}),
        )
        .await;
        assert!(reply.get("error").is_none(), "{reply}");
        waiting.await.unwrap().unwrap();
        originals.push((decision, answer, note, prompt));
    }
    // The first answer reader connects only after all answers have completed.
    let reader = Client::connect(&run.daemon.socket).await.unwrap();
    let records = reader
        .request("decision.answers", json!({"agent":run.child}))
        .await
        .unwrap();
    assert_eq!(records.as_array().unwrap().len(), 3);
    assert_ne!(originals[0].0["id"], originals[1].0["id"]);
    for (record, (decision, answer, note, prompt)) in
        records.as_array().unwrap().iter().zip(&originals)
    {
        assert_eq!(record["decision"], *decision);
        assert_eq!(
            record["kind"],
            if decision["tool"] == "ask_user" {
                "question"
            } else {
                "permission"
            }
        );
        let original: Value = serde_json::from_str(record["prompt"].as_str().unwrap()).unwrap();
        assert_eq!(
            original[if decision["tool"] == "ask_user" {
                "question"
            } else {
                "command"
            }],
            *prompt
        );
        assert_eq!(record["answer"], *answer);
        assert_eq!(record["note"], *note);
        assert_eq!(record["answered_by"]["kind"], "agent");
        assert_eq!(record["answered_by"]["id"], door);
    }
    let outside = door_call(
        &run,
        &door_terminal,
        "decision.answers",
        json!({"agent":unrelated_child}),
    )
    .await;
    assert_eq!(outside["error"]["code"], rpc::code::FORBIDDEN);
    assert_eq!(
        reader
            .request("decision.answers", json!({"agent":unrelated_child}))
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        door_call(
            &run,
            &door_terminal,
            "decision.answers",
            json!({"agent":run.child})
        )
        .await["result"],
        records
    );
    drop(reader);
    let project = run.dir.path().to_owned();
    let data = run.data.path().to_owned();
    drop(run.daemon);
    run.daemon = launch(&project, &data);
    run.user = Arc::new(Client::connect(&run.daemon.socket).await.unwrap());
    assert_eq!(
        run.user
            .request("decision.answers", json!({"agent":run.child}))
            .await
            .unwrap(),
        records
    );
    run.resumed().await;
    let (new_request, waiting) = run.open("permission").await;
    assert!(
        !records
            .as_array()
            .unwrap()
            .iter()
            .any(|record| record["decision"]["id"] == new_request["id"])
    );
    run.user_answer(&new_request, "permission").await;
    waiting.await.unwrap().unwrap();
    let after = run
        .user
        .request("decision.answers", json!({"agent":run.child}))
        .await
        .unwrap();
    assert_eq!(&after.as_array().unwrap()[..3], records.as_array().unwrap());
    assert_eq!(after.as_array().unwrap().len(), 4);
    run.finish().await;
}

#[tokio::test]
async fn u468_a_child_permission_is_answered_in_its_terminal() {
    let run = Run::start().await;
    let (decision, waiting) = run.open("native").await;
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    let kinds: Vec<_> = tree
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["id"] == run.child)
        .map(|node| node["status"]["kind"].clone())
        .collect();
    assert_eq!(kinds, [json!("needs-you")]);
    // The user answers by typing into the child's Terminal, with no Takeover.
    run.user
        .request(
            "terminal.write",
            json!({"id":run.terminal,"data":STANDARD.encode(b"y")}),
        )
        .await
        .unwrap();
    tokio::time::timeout(BOUND, async {
        while !run.screen().await.contains("BYTE:79") {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        run.user.request("decision.list", ()).await.unwrap()[0]["id"],
        decision["id"]
    );
    run.clear().await;
    waiting.await.unwrap().unwrap();
    assert_eq!(
        run.user.request("decision.list", ()).await.unwrap(),
        json!([])
    );
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    assert!(
        tree.as_array()
            .unwrap()
            .iter()
            .filter(|node| node["id"] == run.child)
            .all(|node| node["status"]["kind"] != "needs-you")
    );
    run.finish().await;
}

async fn answer_child() -> (Run, String, Value) {
    let mut run = Run::start().await;
    let door = run
        .user
        .request("rail.createWorkstream", json!({"name":"supervisor"}))
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    run.user
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    run.door = Some(door.clone());
    run.spawn().await;
    let door_node = run
        .user
        .request("rail.startDoor", json!({"id":door}))
        .await
        .unwrap();
    let door_terminal = door_node["terminal_id"].clone();
    run.user
        .request(
            "terminal.write",
            json!({"id":door_terminal,"data":STANDARD.encode(b"\r")}),
        )
        .await
        .unwrap();
    tokio::time::timeout(BOUND, async {
        loop {
            let screen = run
                .user
                .request("terminal.read", json!({"id":door_terminal,"after":0}))
                .await
                .unwrap();
            if screen["text"].as_str().unwrap().contains("READY") {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    (run, door, door_terminal)
}

#[tokio::test]
async fn u468_backend_refused_answers_are_readable_on_the_actionable_original() {
    let (run, door, terminal) = answer_child().await;
    for (kind, refusal) in [
        ("ask_user", "label"),
        ("permission", "word"),
        ("permission", "proof"),
        ("permission", "identity"),
    ] {
        let (decision, waiting) = run.open(kind).await;
        let mut params =
            json!({"id":decision["id"],"answer":if kind == "ask_user" { "Yes" } else { "allow" }});
        let expected = match refusal {
            "label" | "word" => {
                params["answer"] = json!("unlisted");
                rpc::code::INVALID_PARAMS
            }
            "proof" => rpc::code::FORBIDDEN,
            "identity" => {
                let mut identity = run.capture(&decision).await;
                identity["attempt"] = json!("an earlier Attempt");
                params["identity"] = identity;
                rpc::code::CONFLICT
            }
            _ => unreachable!(),
        };
        let reply = if refusal == "proof" {
            // A claimed Door is still an unauthenticated user process.
            run.user
                .request(
                    "daemon.identify",
                    json!({"actor":{"kind":"agent","id":door}}),
                )
                .await
                .unwrap();
            let error = run
                .user
                .request("decision.answer", params)
                .await
                .unwrap_err();
            json!({"error":{"code":error.code}})
        } else {
            door_call(&run, &terminal, "decision.answer", params).await
        };
        if refusal == "proof" {
            run.user
                .request(
                    "daemon.identify",
                    json!({"actor":{"kind":"user","id":"you"}}),
                )
                .await
                .unwrap();
        }
        assert_eq!(reply["error"]["code"], expected, "{refusal}: {reply}");
        let mut listed = run.user.request("decision.list", ()).await.unwrap();
        if matches!(refusal, "label" | "word") {
            let failure = listed[0]
                .as_object_mut()
                .unwrap()
                .remove("failure")
                .unwrap();
            assert_eq!(failure["error_code"], expected);
            assert_eq!(failure["message"], reply["error"]["message"]);
            assert_eq!(
                failure["message"],
                match refusal {
                    "label" => format!(
                        "decision {} takes one of its listed answers, or a note as your own",
                        decision["id"].as_str().unwrap()
                    ),
                    "word" => "answer is allow or deny, with no note".to_owned(),
                    _ => unreachable!(),
                }
            );
            assert_eq!(failure["answered_by"]["kind"], "agent");
            assert_eq!(failure["answered_by"]["id"], door);
            assert_eq!(failure["attempt"], run.attempt);
            assert!(failure["failed_at"].as_i64().unwrap() > 0);
        }
        assert_eq!(listed, json!([decision]));
        let records = run
            .user
            .request("decision.answers", json!({"agent":run.child}))
            .await
            .unwrap();
        assert!(
            !records
                .as_array()
                .unwrap()
                .iter()
                .any(|record| record["decision"]["id"] == decision["id"]),
            "{refusal}: {records}"
        );
        // An ordinary successful retry proves the same request is still actionable.
        run.user_answer(&decision, kind).await;
        waiting.await.unwrap().unwrap();
        let accepted = run
            .user
            .request("decision.answers", json!({"agent":run.child}))
            .await
            .unwrap();
        assert_eq!(
            accepted.as_array().unwrap().last().unwrap()["decision"],
            decision
        );
    }
    run.finish().await;
}

#[tokio::test]
async fn u468_storage_refusal_keeps_the_request_actionable() {
    let (run, _, terminal) = answer_child().await;
    let (decision, waiting) = run.open("permission").await;
    // Inject a storage refusal; all verification still crosses the public Daemon.
    let path = rupd::data::project_data(run.data.path(), run.dir.path())
        .unwrap()
        .join("project.db");
    let db = rusqlite::Connection::open(path).unwrap();
    db.execute_batch("CREATE TRIGGER refuse_answer BEFORE INSERT ON decision_answers BEGIN SELECT RAISE(ABORT, 'storage refused'); END;").unwrap();
    let reply = door_call(
        &run,
        &terminal,
        "decision.answer",
        json!({"id":decision["id"],"answer":"allow"}),
    )
    .await;
    assert_eq!(reply["error"]["code"], rpc::code::INTERNAL);
    let mut listed = run.user.request("decision.list", ()).await.unwrap();
    let failure = listed[0]
        .as_object_mut()
        .unwrap()
        .remove("failure")
        .unwrap();
    assert_eq!(failure["error_code"], rpc::code::INTERNAL);
    assert_eq!(failure["message"], reply["error"]["message"]);
    assert_eq!(listed, json!([decision]));
    assert_eq!(
        run.user
            .request("decision.answers", json!({"agent":run.child}))
            .await
            .unwrap(),
        json!([])
    );
    db.execute_batch("DROP TRIGGER refuse_answer;").unwrap();
    let retry = door_call(
        &run,
        &terminal,
        "decision.answer",
        json!({"id":decision["id"],"answer":"allow"}),
    )
    .await;
    assert!(retry.get("error").is_none(), "{retry}");
    waiting.await.unwrap().unwrap();
    run.finish().await;
}

async fn door_call(run: &Run, terminal: &Value, method: &str, params: Value) -> Value {
    let reply = run.dir.path().join("door-reply.json");
    if reply.exists() {
        std::fs::remove_file(&reply).unwrap();
    }
    std::fs::write(
        run.dir.path().join("door-call.json"),
        json!({"method":method,"params":params}).to_string(),
    )
    .unwrap();
    run.user
        .request(
            "terminal.write",
            json!({"id":terminal,"data":STANDARD.encode([1])}),
        )
        .await
        .unwrap();
    tokio::time::timeout(BOUND, async {
        loop {
            if let Ok(bytes) = std::fs::read(&reply) {
                break serde_json::from_slice(&bytes).unwrap();
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}

async fn ending_takeover_race(method: &'static str) {
    let (run, caller) = if method == "agent.stop" {
        order_child().await
    } else {
        door_terminal().await
    };
    run.pause();
    let id = if method == "agent.stop" {
        json!(run.child)
    } else {
        run.terminal.clone()
    };
    let ending = tokio::spawn(async move { caller.request(method, json!({"id":id})).await });
    run.paused().await;
    let user = Arc::clone(&run.user);
    let child = run.child.clone();
    let mut begin =
        tokio::spawn(async move { user.request("takeover.begin", json!({"agent":child})).await });
    // Arrival acknowledges the paused child effect; this window observes absence of acquisition.
    let early = tokio::time::timeout(Duration::from_millis(200), &mut begin).await;
    run.release();
    tokio::time::timeout(BOUND, ending)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(
        early.is_err(),
        "Takeover acquired before {method}'s child effect: {early:?}"
    );
    let acquired = tokio::time::timeout(BOUND, begin).await.unwrap().unwrap();
    if method == "agent.stop" {
        assert_eq!(acquired.unwrap_err().code, rpc::code::CONFLICT);
    } else {
        // The auxiliary Terminal ended; its owning child remains available for Takeover.
        acquired.unwrap();
    }
    run.finish().await;
}

#[tokio::test]
async fn b6_begin_racing_a_door_stop_waits_for_the_child_effect() {
    ending_takeover_race("agent.stop").await;
}

#[tokio::test]
async fn b6_begin_racing_a_door_terminal_kill_waits_for_the_child_effect() {
    ending_takeover_race("terminal.kill").await;
}

#[tokio::test]
async fn b6_begin_racing_a_door_terminal_close_waits_for_the_child_effect() {
    ending_takeover_race("terminal.close").await;
}

#[tokio::test]
async fn b6_begin_racing_a_door_resize_waits_for_the_child_effect() {
    let (run, caller) = door_terminal().await;
    run.pause();
    let terminal = run.terminal.clone();
    let resize = tokio::spawn(async move {
        caller
            .request(
                "terminal.resize",
                json!({"id":terminal,"cols":101,"rows":37}),
            )
            .await
    });
    run.paused().await;
    let before = run
        .user
        .request("terminal.snapshot", json!({"id":run.terminal}))
        .await
        .unwrap();
    assert_eq!(
        (before["cols"].as_u64(), before["rows"].as_u64()),
        (Some(80), Some(24))
    );
    let user = Arc::clone(&run.user);
    let child = run.child.clone();
    let mut begin =
        tokio::spawn(async move { user.request("takeover.begin", json!({"agent":child})).await });
    // The gate acknowledges arrival before resize. This window observes absence of acquisition.
    let early = tokio::time::timeout(Duration::from_millis(200), &mut begin).await;
    run.release();
    tokio::time::timeout(BOUND, resize)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let acquired_early = early.is_ok();
    let acquired = match early {
        Ok(result) => result.unwrap(),
        Err(_) => tokio::time::timeout(BOUND, begin).await.unwrap().unwrap(),
    };
    let after = run
        .user
        .request("terminal.snapshot", json!({"id":run.terminal}))
        .await
        .unwrap();
    run.finish().await;
    assert!(
        !acquired_early,
        "Takeover acquired before Door resize completed"
    );
    acquired.unwrap();
    assert_eq!(
        (after["cols"].as_u64(), after["rows"].as_u64()),
        (Some(101), Some(37))
    );
}

#[tokio::test]
async fn b6_a_held_resize_is_acknowledged_without_effect_and_released_in_order() {
    let (run, caller) = door_terminal().await;
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let mut accepted = Vec::new();
    for cols in [101, 102] {
        let held = caller
            .request(
                "terminal.resize",
                json!({"id":run.terminal,"cols":cols,"rows":37}),
            )
            .await
            .unwrap();
        assert_eq!(
            held["outcome"],
            json!({"status":"waiting","reason":"takeover"})
        );
        accepted.push(held);
    }
    let before = run
        .user
        .request("terminal.snapshot", json!({"id":run.terminal}))
        .await
        .unwrap();
    assert_eq!(before["cols"], 80);
    let history = caller
        .request("terminal.resizeHistory", json!({"id":run.terminal}))
        .await
        .unwrap();
    assert_eq!(history, json!(accepted));
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let after = run
        .user
        .request("terminal.snapshot", json!({"id":run.terminal}))
        .await
        .unwrap();
    assert_eq!(after["cols"], 102);
    let history = caller
        .request("terminal.resizeHistory", json!({"id":run.terminal}))
        .await
        .unwrap();
    assert_eq!(history.as_array().unwrap().len(), accepted.len());
    for (record, ack) in history.as_array().unwrap().iter().zip(&accepted) {
        assert_eq!(record["id"], ack["id"]);
        assert_eq!(record["outcome"], json!({"status":"completed"}));
    }
    // A duplicate release cannot replay the earlier resize over direct user input.
    run.user
        .request(
            "terminal.resize",
            json!({"id":run.terminal,"cols":111,"rows":38}),
        )
        .await
        .unwrap();
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let after = run
        .user
        .request("terminal.snapshot", json!({"id":run.terminal}))
        .await
        .unwrap();
    assert_eq!(after["cols"], 111);
    run.finish().await;
}

#[tokio::test]
async fn b6_a_later_door_resize_cannot_overtake_released_resizes() {
    let (run, caller) = door_terminal().await;
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    for cols in [101, 102] {
        caller
            .request(
                "terminal.resize",
                json!({"id":run.terminal,"cols":cols,"rows":37}),
            )
            .await
            .unwrap();
    }
    run.pause();
    let user = Arc::clone(&run.user);
    let child = run.child.clone();
    let release =
        tokio::spawn(async move { user.request("takeover.end", json!({"agent":child})).await });
    run.paused().await;
    let terminal = run.terminal.clone();
    let mut later = tokio::spawn(async move {
        caller
            .request(
                "terminal.resize",
                json!({"id":terminal,"cols":103,"rows":39}),
            )
            .await
    });
    let early = tokio::time::timeout(Duration::from_millis(200), &mut later).await;
    run.release();
    release.await.unwrap().unwrap();
    later.await.unwrap().unwrap();
    let after = run
        .user
        .request("terminal.snapshot", json!({"id":run.terminal}))
        .await
        .unwrap();
    assert_eq!(after["cols"], 103);
    assert!(
        early.is_err(),
        "a later resize overtook the paused release: {early:?}"
    );
    run.finish().await;
}

#[tokio::test]
async fn b6_resize_waits_leave_user_input_other_children_and_messages_available() {
    let (run, caller) = door_terminal().await;
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let held = caller
        .request(
            "terminal.resize",
            json!({"id":run.terminal,"cols":101,"rows":37}),
        )
        .await
        .unwrap();
    assert_eq!(held["outcome"]["status"], "waiting");
    let message = run
        .user
        .request(
            "message.send",
            json!({"to":run.child,"kind":"note","body":"held independently"}),
        )
        .await
        .unwrap();
    assert_eq!(message["status"], "held");
    assert_eq!(message["reason"], "takeover");
    run.user
        .request(
            "terminal.resize",
            json!({"id":run.terminal,"cols":111,"rows":38}),
        )
        .await
        .unwrap();
    let before = run
        .user
        .request("terminal.snapshot", json!({"id":run.terminal}))
        .await
        .unwrap();
    assert_eq!(before["cols"], 111);
    let other = caller
        .request("agent.spawn", json!({"name":"other","cwd":run.dir.path()}))
        .await
        .unwrap();
    // A second child-owned Terminal remains accessible while the first child is held.
    let owner = Client::connect(&run.daemon.socket).await.unwrap();
    owner
        .request(
            "daemon.identify",
            json!({"actor":{"kind":"agent","id":other["id"]}}),
        )
        .await
        .unwrap();
    let other_terminal = owner
        .request(
            "terminal.spawn",
            json!({"cwd":run.dir.path(),"cols":80,"rows":24,"env":{},"command":["sh","-c","cat"]}),
        )
        .await
        .unwrap();
    let result = caller
        .request(
            "terminal.resize",
            json!({"id":other_terminal["id"],"cols":105,"rows":35}),
        )
        .await
        .unwrap();
    assert!(result.is_null(), "another child was held: {result}");
    let snapshot = run
        .user
        .request("terminal.snapshot", json!({"id":other_terminal["id"]}))
        .await
        .unwrap();
    assert_eq!(snapshot["cols"], 105);
    let unrelated = Client::connect(&run.daemon.socket).await.unwrap();
    let foreign = run
        .user
        .request("rail.createWorkstream", json!({"name":"foreign"}))
        .await
        .unwrap();
    unrelated
        .request(
            "daemon.identify",
            json!({"actor":{"kind":"agent","id":foreign["id"]}}),
        )
        .await
        .unwrap();
    let denied = unrelated
        .request(
            "terminal.resize",
            json!({"id":run.terminal,"cols":106,"rows":35}),
        )
        .await
        .unwrap_err();
    assert_eq!(denied.code, rpc::code::FORBIDDEN);
    assert_eq!(
        unrelated
            .request("terminal.resizeHistory", json!({"id":run.terminal}))
            .await
            .unwrap(),
        json!([])
    );
    let invalid = caller
        .request(
            "terminal.resize",
            json!({"id":run.terminal,"cols":0,"rows":37}),
        )
        .await
        .unwrap_err();
    assert_eq!(invalid.code, rpc::code::INVALID_PARAMS);
    assert_eq!(
        caller
            .request("terminal.resizeHistory", json!({"id":run.terminal}))
            .await
            .unwrap(),
        json!([held])
    );
    run.user
        .request("message.drop", json!({"id":message["id"]}))
        .await
        .unwrap();
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let message = run
        .user
        .request("message.get", json!({"id":message["id"]}))
        .await
        .unwrap();
    assert_eq!(message["status"], "dropped");
    for (method, id) in [
        ("terminal.close", other_terminal["id"].clone()),
        ("agent.stop", other["id"].clone()),
    ] {
        run.user.request(method, json!({"id":id})).await.unwrap();
    }
    run.finish().await;
}

#[tokio::test]
async fn b6_an_unavailable_resize_is_recorded_and_does_not_block_the_next_terminal() {
    let (run, caller) = door_terminal().await;
    let child = Client::connect(&run.daemon.socket).await.unwrap();
    child
        .request(
            "daemon.identify",
            json!({"actor":{"kind":"agent","id":run.child}}),
        )
        .await
        .unwrap();
    let second = child
        .request(
            "terminal.spawn",
            json!({"cwd":run.dir.path(),"cols":80,"rows":24,"env":{},"command":["sh","-c","cat"]}),
        )
        .await
        .unwrap();
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    caller
        .request(
            "terminal.resize",
            json!({"id":run.terminal,"cols":101,"rows":37}),
        )
        .await
        .unwrap();
    caller
        .request(
            "terminal.resize",
            json!({"id":second["id"],"cols":102,"rows":38}),
        )
        .await
        .unwrap();
    run.user
        .request("terminal.close", json!({"id":run.terminal}))
        .await
        .unwrap();
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let dropped = caller
        .request("terminal.resizeHistory", json!({"id":run.terminal}))
        .await
        .unwrap();
    assert_eq!(dropped[0]["outcome"]["status"], "dropped");
    assert!(!dropped[0]["outcome"]["reason"].as_str().unwrap().is_empty());
    let completed = caller
        .request("terminal.resizeHistory", json!({"id":second["id"]}))
        .await
        .unwrap();
    assert_eq!(completed[0]["outcome"]["status"], "completed");
    let snapshot = run
        .user
        .request("terminal.snapshot", json!({"id":second["id"]}))
        .await
        .unwrap();
    assert_eq!(snapshot["cols"], 102);
    run.user
        .request("terminal.close", json!({"id":second["id"]}))
        .await
        .unwrap();
    run.finish().await;
}

#[tokio::test]
async fn b6_a_child_exit_accounts_for_its_held_resizes_without_a_release() {
    let (run, caller) = door_terminal().await;
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    caller
        .request(
            "terminal.resize",
            json!({"id":run.terminal,"cols":101,"rows":37}),
        )
        .await
        .unwrap();
    run.user
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    let actions = caller
        .request("terminal.resizeHistory", json!({"id":run.terminal}))
        .await
        .unwrap();
    assert_eq!(actions[0]["outcome"]["status"], "dropped");
    assert!(
        actions[0]["outcome"]["reason"]
            .as_str()
            .unwrap()
            .contains("exited")
    );
    let snapshot = run
        .user
        .request("terminal.snapshot", json!({"id":run.terminal}))
        .await
        .unwrap();
    assert_eq!(snapshot["cols"], 80);
    run.user
        .request("terminal.close", json!({"id":run.terminal}))
        .await
        .unwrap();
}

#[tokio::test]
async fn b6_resize_crashes_preserve_receipts_and_never_replay_an_uncertain_effect() {
    for boundary in ["waiting", "before", "after-resize", "completed"] {
        let (mut run, caller) = door_terminal().await;
        let terminal = run.terminal.clone();
        let tree = run.user.request("rail.tree", ()).await.unwrap();
        let door = tree
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == run.child)
            .unwrap()["parent"]
            .clone();
        run.user
            .request("takeover.begin", json!({"agent":run.child}))
            .await
            .unwrap();
        let receipt = caller
            .request(
                "terminal.resize",
                json!({"id":terminal,"cols":101,"rows":37}),
            )
            .await
            .unwrap();
        let release = if boundary == "waiting" {
            None
        } else {
            if boundary != "completed" {
                std::fs::write(run.dir.path().join("effect-gate"), boundary).unwrap();
            }
            let user = Arc::clone(&run.user);
            let child = run.child.clone();
            Some(tokio::spawn(async move {
                user.request("takeover.end", json!({"agent":child})).await
            }))
        };
        if boundary == "completed" {
            release.unwrap().await.unwrap().unwrap();
        } else if let Some(release) = release {
            run.paused().await;
            let snapshot = run
                .user
                .request("terminal.snapshot", json!({"id":terminal}))
                .await
                .unwrap();
            assert_eq!(
                snapshot["cols"],
                if boundary == "before" { 80 } else { 101 }
            );
            run.daemon.child.kill().unwrap();
            run.daemon.child.wait().unwrap();
            assert!(release.await.unwrap().is_err());
        }
        if run.daemon.child.try_wait().unwrap().is_none() {
            run.daemon.child.kill().unwrap();
            run.daemon.child.wait().unwrap();
        }
        let _ = std::fs::remove_file(run.dir.path().join("effect-gate"));
        run.daemon = launch(run.dir.path(), run.data.path());
        run.user = Arc::new(Client::connect(&run.daemon.socket).await.unwrap());
        let caller = Client::connect(&run.daemon.socket).await.unwrap();
        caller
            .request(
                "daemon.identify",
                json!({"actor":{"kind":"agent","id":door}}),
            )
            .await
            .unwrap();
        let history = caller
            .request("terminal.resizeHistory", json!({"id":terminal}))
            .await
            .unwrap();
        assert_eq!(history.as_array().unwrap().len(), 1);
        assert_eq!(history[0]["id"], receipt["id"]);
        assert_eq!(
            run.user
                .request("terminal.resizeHistory", json!({"id":terminal}))
                .await
                .unwrap(),
            history
        );
        if boundary == "completed" {
            assert_eq!(history[0]["outcome"]["status"], "completed");
        } else {
            assert_eq!(history[0]["outcome"]["status"], "dropped");
            let reason = history[0]["outcome"]["reason"].as_str().unwrap();
            assert!(
                reason.contains(if boundary == "waiting" {
                    "did not survive"
                } else {
                    "may have happened"
                }),
                "{boundary}: {reason}"
            );
        }
        // Restart clears Takeover; an explicitly resumed child accepts a new Takeover.
        run.resumed().await;
        run.user
            .request("takeover.begin", json!({"agent":run.child}))
            .await
            .unwrap();
        run.user
            .request("takeover.end", json!({"agent":run.child}))
            .await
            .unwrap();
        assert_eq!(
            caller
                .request("terminal.resizeHistory", json!({"id":terminal}))
                .await
                .unwrap(),
            history
        );
        run.finish().await;
    }
}

#[tokio::test]
async fn b6_held_resizes_preserve_wire_arrival_order_before_task_scheduling() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let (run, _) = door_terminal().await;
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    let door = tree
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == run.child)
        .unwrap()["parent"]
        .clone();
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let mut client = tokio::net::UnixStream::connect(&run.daemon.socket)
        .await
        .unwrap();
    let mut batch = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"method":"daemon.identify","params":{"actor":{"kind":"agent","id":door}}})).unwrap();
    batch.push(b'\n');
    for cols in 120..152 {
        batch.extend(serde_json::to_vec(&json!({"jsonrpc":"2.0","id":cols,"method":"terminal.resize","params":{"id":run.terminal,"cols":cols,"rows":37}})).unwrap());
        batch.push(b'\n');
    }
    client.write_all(&batch).await.unwrap();
    client.shutdown().await.unwrap();
    let mut replies = BufReader::new(client).lines();
    for _ in 0..33 {
        let line = tokio::time::timeout(BOUND, replies.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let reply: Value = serde_json::from_str(&line).unwrap();
        assert!(reply.get("result").is_some(), "{reply}");
    }
    let history = run
        .user
        .request("terminal.resizeHistory", json!({"id":run.terminal}))
        .await
        .unwrap();
    let widths: Vec<_> = history
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["cols"].as_u64().unwrap())
        .collect();
    assert_eq!(
        widths,
        (120..152).collect::<Vec<_>>(),
        "acceptance must follow the already-read wire frames"
    );
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let snapshot = run
        .user
        .request("terminal.snapshot", json!({"id":run.terminal}))
        .await
        .unwrap();
    assert_eq!(snapshot["cols"], 151);
    run.finish().await;
}

#[tokio::test]
async fn b6_a_new_takeover_does_not_wait_for_an_end_delivering_a_recorded_message() {
    let (run, caller) = door_terminal().await;
    std::fs::write(run.dir.path().join("order-submit"), "accept").unwrap();
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    caller
        .request(
            "terminal.resize",
            json!({"id":run.terminal,"cols":101,"rows":37}),
        )
        .await
        .unwrap();
    let message = run
        .user
        .request(
            "message.send",
            json!({"to":run.child,"kind":"note","body":"legacy recorded cut"}),
        )
        .await
        .unwrap();
    let gate = run.dir.path().join("input-gate");
    std::fs::write(&gate, "pause").unwrap();
    let user = Arc::clone(&run.user);
    let child = run.child.clone();
    let end =
        tokio::spawn(async move { user.request("takeover.end", json!({"agent":child})).await });
    tokio::time::timeout(BOUND, async {
        while !gate.with_extension("arrived").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let user = Arc::clone(&run.user);
    let child = run.child.clone();
    let mut begin =
        tokio::spawn(async move { user.request("takeover.begin", json!({"agent":child})).await });
    let early = tokio::time::timeout(Duration::from_secs(1), &mut begin).await;
    std::fs::remove_file(gate).unwrap();
    end.await.unwrap().unwrap();
    let acquired_early = early.is_ok();
    match early {
        Ok(result) => {
            result.unwrap().unwrap();
        }
        Err(_) => {
            begin.await.unwrap().unwrap();
        }
    }
    let record = run
        .user
        .request("message.get", json!({"id":message["id"]}))
        .await
        .unwrap();
    assert_eq!(record["status"], "delivered");
    let snapshot = run
        .user
        .request("terminal.snapshot", json!({"id":run.terminal}))
        .await
        .unwrap();
    assert!(
        acquired_early,
        "begin waited for the earlier end's Message typing"
    );
    assert_eq!(
        snapshot["cols"], 80,
        "the earlier end released a resize into the new Takeover"
    );
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let snapshot = run
        .user
        .request("terminal.snapshot", json!({"id":run.terminal}))
        .await
        .unwrap();
    assert_eq!(snapshot["cols"], 101);
    run.finish().await;
}

#[tokio::test]
async fn b6_begin_racing_a_door_rename_waits_for_the_child_effect() {
    let (run, caller) = order_child().await;
    run.pause();
    let child = run.child.clone();
    let rename = tokio::spawn(async move {
        caller
            .request(
                "rail.rename",
                json!({"id":child,"name":"renamed before Takeover"}),
            )
            .await
    });
    run.paused().await;
    let user = Arc::clone(&run.user);
    let child = run.child.clone();
    let mut begin =
        tokio::spawn(async move { user.request("takeover.begin", json!({"agent":child})).await });
    // The observer acknowledges arrival before the rename. This window checks no acquisition.
    let acquired_early = tokio::time::timeout(Duration::from_millis(200), &mut begin)
        .await
        .is_ok();
    run.release();
    let renamed = tokio::time::timeout(BOUND, rename)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    if !acquired_early {
        tokio::time::timeout(BOUND, begin)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
    run.finish().await;
    assert!(
        !acquired_early,
        "Takeover acquired before Door rename completed"
    );
    assert_eq!(renamed["name"], "renamed before Takeover");
}

#[tokio::test]
async fn b6_later_door_effects_cannot_overtake_a_released_resize() {
    for method in [
        "rail.rename",
        "rail.move",
        "rail.remove",
        "agent.discard",
        "agent.land",
        "agent.worktreeState",
        "agent.run",
        "agent.spawnPrompt",
        "agent.digest",
        "terminal.kill",
        "terminal.snapshot",
        "terminal.screen",
        "terminal.read",
        "terminal.search",
    ] {
        let (run, caller) = door_terminal().await;
        std::fs::write(run.dir.path().join("order-submit"), "accept").unwrap();
        let digest_door = if method == "agent.digest" {
            let door = start_answer_door(&run).await.0;
            let tree = run.user.request("rail.tree", ()).await.unwrap();
            let stopped = tree
                .as_array()
                .unwrap()
                .iter()
                .find(|n| n["kind"] == "agent" && n["parent"].is_null())
                .unwrap();
            run.user
                .request(
                    "rail.move",
                    json!({"id":stopped["id"],"parent":door,"index":0}),
                )
                .await
                .unwrap();
            Some(door)
        } else {
            None
        };
        run.user
            .request("takeover.begin", json!({"agent":run.child}))
            .await
            .unwrap();
        caller
            .request(
                "terminal.resize",
                json!({"id":run.terminal,"cols":101,"rows":37}),
            )
            .await
            .unwrap();
        run.user
            .request(
                "message.send",
                json!({"to":run.child,"kind":"note","body":"release ordering"}),
            )
            .await
            .unwrap();
        let mut events = notice_events(&run).await;
        let gate = run.dir.path().join("input-gate");
        std::fs::write(&gate, "pause").unwrap();
        let user = Arc::clone(&run.user);
        let child = run.child.clone();
        let end =
            tokio::spawn(async move { user.request("takeover.end", json!({"agent":child})).await });
        // A background Digest can reach the shared input gate before the child is released.
        // Observe the child's Takeover boundary independently of the physical input gate.
        tokio::time::timeout(BOUND, async {
            loop {
                if matches!(events.next_event().await.unwrap().data,
                    EventData::TakeoverChanged(ref change)
                    if change.agent == run.child && !change.on)
                {
                    break;
                }
            }
        })
        .await
        .unwrap();
        tokio::time::timeout(BOUND, async {
            while !gate.with_extension("arrived").is_file() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let params = match method {
            "agent.digest" => json!({"id":digest_door}),
            "rail.rename" => json!({"id":run.child,"name":"after resize"}),
            "agent.run" => json!({"id":run.child,"command":["/bin/true"]}),
            "rail.move" => json!({"id":run.child,"parent":null,"index":0}),
            "rail.remove"
            | "agent.discard"
            | "agent.land"
            | "agent.worktreeState"
            | "agent.spawnPrompt" => {
                json!({"id":run.child})
            }
            "terminal.read" => json!({"id":run.terminal,"after":0}),
            "terminal.search" => json!({"id":run.terminal,"pattern":"READY"}),
            _ => json!({"id":run.terminal}),
        };
        let effect = tokio::time::timeout(BOUND, caller.request(method, params)).await;
        let history = tokio::time::timeout(
            BOUND,
            run.user
                .request("terminal.resizeHistory", json!({"id":run.terminal})),
        )
        .await;
        std::fs::remove_file(gate).unwrap();
        end.await.unwrap().unwrap();
        if method == "rail.remove" {
            drop(run);
        } else {
            run.finish().await;
        }
        let effect = effect.unwrap();
        if matches!(
            method,
            "agent.discard" | "agent.land" | "agent.worktreeState"
        ) {
            assert_eq!(effect.unwrap_err().code, rpc::code::NOT_FOUND);
        } else {
            effect.unwrap();
        }
        assert_eq!(
            history.unwrap().unwrap()[0]["outcome"]["status"],
            "completed",
            "{method} returned before the older resize ran"
        );
    }
}

#[tokio::test]
async fn b6_wire_later_door_effects_wait_for_all_earlier_resizes() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    for method in [
        "terminal.snapshot",
        "terminal.screen",
        "terminal.read",
        "terminal.search",
        "terminal.write",
        "agent.prompt",
        "agent.setOrder",
        "agent.interrupt",
        "agent.resume",
        "agent.stop",
        "terminal.kill",
        "terminal.close",
        "rail.rename",
        "rail.move",
        "rail.remove",
        "agent.discard",
        "agent.land",
        "agent.worktreeState",
        "agent.run",
        "agent.spawnPrompt",
        "agent.digest",
    ] {
        let (run, _) = door_terminal().await;
        std::fs::write(run.dir.path().join("order-submit"), "accept").unwrap();
        if method == "agent.digest" {
            start_answer_door(&run).await;
        }
        let tree = run.user.request("rail.tree", ()).await.unwrap();
        let door = tree
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == run.child)
            .unwrap()["parent"]
            .clone();
        if method == "agent.digest" {
            let stopped = tree
                .as_array()
                .unwrap()
                .iter()
                .find(|n| n["kind"] == "agent" && n["parent"].is_null())
                .unwrap();
            run.user
                .request(
                    "rail.move",
                    json!({"id":stopped["id"],"parent":door,"index":0}),
                )
                .await
                .unwrap();
        }
        let client = tokio::net::UnixStream::connect(&run.daemon.socket)
            .await
            .unwrap();
        let (read, mut write) = client.into_split();
        let mut replies = BufReader::new(read).lines();
        write.write_all(format!("{}\n", json!({"id":1,"method":"daemon.identify","params":{"actor":{"kind":"agent","id":door}}})).as_bytes()).await.unwrap();
        let identified: Value = serde_json::from_str(
            &tokio::time::timeout(BOUND, replies.next_line())
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert!(identified.get("result").is_some(), "{identified}");
        run.pause();
        write.write_all(format!("{}\n", json!({"id":2,"method":"terminal.resize","params":{"id":run.terminal,"cols":101,"rows":37}})).as_bytes()).await.unwrap();
        run.paused().await;
        let mut batch = Vec::new();
        for cols in 102..110 {
            batch.extend(format!("{}\n", json!({"id":cols,"method":"terminal.resize","params":{"id":run.terminal,"cols":cols,"rows":37}})).as_bytes());
        }
        let params = match method {
            "terminal.snapshot" | "terminal.screen" => json!({"id":run.terminal}),
            "terminal.read" => json!({"id":run.terminal,"after":0}),
            "terminal.search" => json!({"id":run.terminal,"pattern":"READY"}),
            "terminal.write" => json!({"id":run.terminal,"data":STANDARD.encode(b"later")}),
            "terminal.kill" | "terminal.close" => json!({"id":run.terminal}),
            "agent.prompt" => json!({"id":run.child,"text":"later prompt"}),
            "agent.setOrder" => {
                json!({"id":run.child,"order":{"kind":"work","ask":"later order","limits":[]}})
            }
            "agent.digest" => json!({"id":door}),
            "rail.rename" => json!({"id":run.child,"name":"after every resize"}),
            "agent.run" => json!({"id":run.child,"command":["/bin/true"]}),
            "rail.move" => json!({"id":run.child,"parent":null,"index":0}),
            _ => json!({"id":run.child}),
        };
        batch.extend(format!("{}\n", json!({"id":110,"method":method,"params":params})).as_bytes());
        // This reply acknowledges that the reader has scheduled every earlier wire frame.
        batch.extend(format!("{}\n", json!({"id":111,"method":"rail.tree"})).as_bytes());
        write.write_all(&batch).await.unwrap();
        let barrier: Value = serde_json::from_str(
            &tokio::time::timeout(BOUND, replies.next_line())
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        if barrier["id"] != 111 {
            run.release();
            if method == "rail.remove" {
                drop(run);
            } else {
                run.finish().await;
            }
            panic!("{method} overtook earlier resizes before the wire barrier: {barrier}");
        }
        run.release();
        let mut order = Vec::new();
        for _ in 0..10 {
            let reply: Value = serde_json::from_str(
                &tokio::time::timeout(BOUND, replies.next_line())
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap(),
            )
            .unwrap();
            if reply["id"] == 110
                && matches!(
                    method,
                    "agent.discard" | "agent.land" | "agent.worktreeState"
                )
            {
                assert_eq!(reply["error"]["code"], rpc::code::NOT_FOUND, "{reply}");
            } else if reply["id"] == 110 && method == "agent.resume" {
                assert_eq!(reply["error"]["code"], rpc::code::CONFLICT, "{reply}");
            } else if reply["id"] == 110 && method == "agent.interrupt" {
                // A fake Claude need not acknowledge Escape; its accounted unknown is final.
                assert!(
                    reply.get("result").is_some()
                        || (reply["error"]["code"] == rpc::code::UNKNOWN_OUTCOME
                            || reply["error"]["code"] == rpc::code::NOT_RUNNING),
                    "{reply}"
                );
            } else {
                assert!(reply.get("result").is_some(), "{method}: {reply}");
            }
            if reply["id"] == 110 && matches!(method, "terminal.snapshot" | "terminal.screen") {
                assert_eq!(
                    reply["result"]["cols"], 109,
                    "read observed stale dimensions"
                );
                assert_eq!(reply["result"]["rows"], 37);
            }
            order.push(reply["id"].as_u64().unwrap());
        }
        if method == "rail.remove" {
            drop(run);
        } else {
            run.finish().await;
        }
        assert_eq!(
            order,
            [2, 102, 103, 104, 105, 106, 107, 108, 109, 110],
            "{method} overtook an already-arrived resize"
        );
    }
}

#[tokio::test]
async fn b6_child_snapshots_keep_reach_restrictions() {
    let (run, door) = door_terminal().await;
    let foreign = run
        .user
        .request("rail.createWorkstream", json!({"name":"unrelated"}))
        .await
        .unwrap();
    let unrelated = Client::connect(&run.daemon.socket).await.unwrap();
    unrelated
        .request(
            "daemon.identify",
            json!({"actor":{"kind":"agent","id":foreign["id"]}}),
        )
        .await
        .unwrap();
    let allowed = door
        .request("terminal.snapshot", json!({"id":run.terminal}))
        .await;
    let denied = unrelated
        .request("terminal.snapshot", json!({"id":run.terminal}))
        .await;
    let absent = door
        .request("terminal.snapshot", json!({"id":"999999"}))
        .await;
    run.finish().await;
    assert_eq!(allowed.unwrap()["cols"], 80);
    assert_eq!(denied.unwrap_err().code, rpc::code::FORBIDDEN);
    assert_eq!(absent.unwrap_err().code, rpc::code::NOT_FOUND);
}

#[tokio::test]
async fn b6_takeover_holds_a_door_rename_without_blocking_user_rename() {
    let (run, caller) = door_terminal().await;
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let accepted = caller
        .request("rail.rename", json!({"id":run.child,"name":"Door rename"}))
        .await;
    let before = run
        .user
        .request("rail.tree", ())
        .await
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == run.child)
        .unwrap()
        .clone();
    run.user
        .request("rail.rename", json!({"id":run.child,"name":"User rename"}))
        .await
        .unwrap();
    let user_name = run
        .user
        .request("rail.tree", ())
        .await
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == run.child)
        .unwrap()
        .clone()["name"]
        .clone();
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let after = run
        .user
        .request("rail.tree", ())
        .await
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == run.child)
        .unwrap()
        .clone();
    run.finish().await;
    assert_eq!(
        accepted.unwrap()["outcome"],
        json!({"status":"waiting","reason":"takeover"})
    );
    assert_ne!(before["name"], "Door rename");
    assert_eq!(user_name, "User rename");
    assert_eq!(after["name"], "Door rename");
}

async fn child_name(run: &Run, id: &Value) -> Value {
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    tree.as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == *id)
        .unwrap()["name"]
        .clone()
}

#[tokio::test]
async fn u467_giving_back_releases_only_that_childs_door_actions() {
    let (run, caller) = door_terminal().await;
    let other = caller
        .request(
            "agent.spawn",
            json!({"name":"other child","cwd":run.dir.path()}),
        )
        .await
        .unwrap();
    tokio::time::timeout(BOUND, async {
        loop {
            let output = run
                .user
                .request(
                    "terminal.read",
                    json!({"id":other["terminal_id"],"after":0}),
                )
                .await
                .unwrap();
            if output["text"].as_str().unwrap().contains("READY") {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let first = json!(run.child);
    for id in [&first, &other["id"]] {
        run.user
            .request("takeover.begin", json!({"agent":id}))
            .await
            .unwrap();
    }
    for (id, name) in [(&first, "first held"), (&other["id"], "other held")] {
        let receipt = caller
            .request("rail.rename", json!({"id":id,"name":name}))
            .await
            .unwrap();
        assert_eq!(
            receipt["outcome"],
            json!({"status":"waiting","reason":"takeover"})
        );
    }
    run.user
        .request("takeover.end", json!({"agent":first}))
        .await
        .unwrap();
    let released = child_name(&run, &first).await;
    let still_held = child_name(&run, &other["id"]).await;
    run.user
        .request("takeover.end", json!({"agent":other["id"]}))
        .await
        .unwrap();
    let then_released = child_name(&run, &other["id"]).await;
    run.user
        .request("agent.stop", json!({"id":other["id"]}))
        .await
        .unwrap();
    run.finish().await;
    assert_eq!(released, "first held");
    assert_ne!(still_held, "other held");
    assert_eq!(then_released, "other held");
}

#[tokio::test]
async fn b6_held_renames_share_resize_order_at_most_once() {
    let (run, caller) = door_terminal().await;
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let mut receipts = Vec::new();
    for (method, params) in [
        (
            "terminal.resize",
            json!({"id":run.terminal,"cols":101,"rows":37}),
        ),
        ("rail.rename", json!({"id":run.child,"name":"first rename"})),
        (
            "terminal.resize",
            json!({"id":run.terminal,"cols":102,"rows":38}),
        ),
        ("rail.rename", json!({"id":run.child,"name":"last rename"})),
    ] {
        receipts.push(caller.request(method, params).await.unwrap());
    }
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    assert_eq!(
        history
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["id"].clone())
            .collect::<Vec<_>>(),
        receipts.iter().map(|a| a["id"].clone()).collect::<Vec<_>>()
    );
    assert!(
        history
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["outcome"]["status"] == "waiting")
    );
    std::fs::write(run.dir.path().join("effect-gate"), "after-rename").unwrap();
    let child = run.child.clone();
    let user = Arc::clone(&run.user);
    let end =
        tokio::spawn(async move { user.request("takeover.end", json!({"agent":child})).await });
    run.paused().await;
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    assert_eq!(
        tree.as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == run.child)
            .unwrap()["name"],
        "first rename"
    );
    assert_eq!(
        run.user
            .request("terminal.snapshot", json!({"id":run.terminal}))
            .await
            .unwrap()["cols"],
        101
    );
    run.release();
    end.await.unwrap().unwrap();
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    assert_eq!(history.as_array().unwrap().len(), 4);
    assert!(
        history
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["outcome"]["status"] == "completed")
    );
    assert_eq!(
        run.user
            .request("agent.actionHistory", json!({"id":run.child}))
            .await
            .unwrap(),
        history
    );
    assert_eq!(
        run.user
            .request("terminal.snapshot", json!({"id":run.terminal}))
            .await
            .unwrap()["cols"],
        102
    );
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    assert_eq!(
        tree.as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == run.child)
            .unwrap()["name"],
        "last rename"
    );
    run.user
        .request(
            "rail.rename",
            json!({"id":run.child,"name":"user after release"}),
        )
        .await
        .unwrap();
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    assert_eq!(
        tree.as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == run.child)
            .unwrap()["name"],
        "user after release"
    );
    run.finish().await;
}

#[tokio::test]
async fn b6_a_held_child_action_losing_reach_drops_and_the_new_doors_rename_proceeds() {
    for method in ["rail.rename", "agent.stop"] {
        let (run, caller) = door_terminal().await;
        run.user
            .request("takeover.begin", json!({"agent":run.child}))
            .await
            .unwrap();
        caller
            .request(method, json!({"id":run.child,"name":"must not rename"}))
            .await
            .unwrap();
        let other = run
            .user
            .request("rail.createWorkstream", json!({"name":"new owner"}))
            .await
            .unwrap();
        run.user
            .request(
                "rail.move",
                json!({"id":run.child,"parent":other["id"],"index":0}),
            )
            .await
            .unwrap();
        let new_door = Client::connect(&run.daemon.socket).await.unwrap();
        new_door
            .request(
                "daemon.identify",
                json!({"actor":{"kind":"agent","id":other["id"]}}),
            )
            .await
            .unwrap();
        let later = new_door
            .request(
                "rail.rename",
                json!({"id":run.child,"name":"new Doors rename"}),
            )
            .await
            .unwrap();
        assert_eq!(later["outcome"]["status"], "waiting");
        run.user
            .request("takeover.end", json!({"agent":run.child}))
            .await
            .unwrap();
        let history = run
            .user
            .request("agent.actionHistory", json!({"id":run.child}))
            .await
            .unwrap();
        assert_eq!(history.as_array().unwrap().len(), 2);
        assert_eq!(history[0]["outcome"]["status"], "dropped");
        assert!(
            history[0]["outcome"]["reason"]
                .as_str()
                .unwrap()
                .contains("not in the Reach")
        );
        assert_eq!(history[1]["outcome"]["status"], "completed");
        let tree = run.user.request("rail.tree", ()).await.unwrap();
        assert_eq!(
            tree.as_array()
                .unwrap()
                .iter()
                .find(|n| n["id"] == run.child)
                .unwrap()["name"],
            "new Doors rename"
        );
        assert_eq!(
            caller
                .request("agent.actionHistory", json!({"id":run.child}))
                .await
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        run.finish().await;
    }
}

#[tokio::test]
async fn b6_rename_crashes_replay_waiting_but_never_repeat_started_effects() {
    for boundary in ["waiting", "before", "after-rename", "completed"] {
        let (mut run, caller) = door_terminal().await;
        let tree = run.user.request("rail.tree", ()).await.unwrap();
        let door = tree
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == run.child)
            .unwrap()["parent"]
            .clone();
        run.user
            .request("takeover.begin", json!({"agent":run.child}))
            .await
            .unwrap();
        let receipt = caller
            .request(
                "rail.rename",
                json!({"id":run.child,"name":"durable rename"}),
            )
            .await
            .unwrap();
        if boundary != "waiting" {
            if boundary != "completed" {
                std::fs::write(run.dir.path().join("effect-gate"), boundary).unwrap();
            }
            let user = Arc::clone(&run.user);
            let child = run.child.clone();
            let end =
                tokio::spawn(
                    async move { user.request("takeover.end", json!({"agent":child})).await },
                );
            if boundary == "completed" {
                end.await.unwrap().unwrap();
            } else {
                run.paused().await;
                run.daemon.child.kill().unwrap();
                run.daemon.child.wait().unwrap();
                assert!(end.await.unwrap().is_err());
            }
        }
        if run.daemon.child.try_wait().unwrap().is_none() {
            run.daemon.child.kill().unwrap();
            run.daemon.child.wait().unwrap();
        }
        let _ = std::fs::remove_file(run.dir.path().join("effect-gate"));
        run.daemon = launch(run.dir.path(), run.data.path());
        run.user = Arc::new(Client::connect(&run.daemon.socket).await.unwrap());
        // Ping at launch follows startup reconciliation, before anyone reads action history.
        let tree = run.user.request("rail.tree", ()).await.unwrap();
        let name = &tree
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == run.child)
            .unwrap()["name"];
        assert_eq!(name == "durable rename", boundary != "before", "{boundary}");
        let history = run
            .user
            .request("agent.actionHistory", json!({"id":run.child}))
            .await
            .unwrap();
        assert_eq!(history.as_array().unwrap().len(), 1);
        assert_eq!(history[0]["id"], receipt["id"]);
        if matches!(boundary, "before" | "after-rename") {
            assert_eq!(history[0]["outcome"]["status"], "dropped");
            assert!(
                history[0]["outcome"]["reason"]
                    .as_str()
                    .unwrap()
                    .contains("may have happened")
            );
        } else {
            assert_eq!(history[0]["outcome"]["status"], "completed");
        }
        let touches = run
            .user
            .request(
                "provenance.history",
                json!({"item":format!("rail:{}",run.child)}),
            )
            .await
            .unwrap();
        let effects = touches
            .as_array()
            .unwrap()
            .iter()
            .filter(|t| t["actor"]["id"] == door && t["verb"] == "wrote")
            .count();
        assert_eq!(
            effects,
            usize::from(boundary != "before"),
            "{boundary}: {touches}"
        );
        run.resumed().await;
        run.user
            .request(
                "rail.rename",
                json!({"id":run.child,"name":"after restart"}),
            )
            .await
            .unwrap();
        run.user
            .request("takeover.end", json!({"agent":run.child}))
            .await
            .unwrap();
        let tree = run.user.request("rail.tree", ()).await.unwrap();
        assert_eq!(
            tree.as_array()
                .unwrap()
                .iter()
                .find(|n| n["id"] == run.child)
                .unwrap()["name"],
            "after restart"
        );
        run.finish().await;
    }
}

#[tokio::test]
async fn b6_a_child_exit_releases_its_valid_held_rename_without_a_history_read() {
    let (run, caller) = door_terminal().await;
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    let door = tree
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == run.child)
        .unwrap()["parent"]
        .clone();
    let mut events = notice_events(&run).await;
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    caller
        .request(
            "rail.rename",
            json!({"id":run.child,"name":"rename after exit"}),
        )
        .await
        .unwrap();
    run.user
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    tokio::time::timeout(BOUND, async {
        loop {
            let event = events.next_event().await.expect("exit observer closed");
            if event.actor.id == door && matches!(event.data, EventData::RailChanged) {
                break;
            }
        }
    })
    .await
    .unwrap();
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    assert_eq!(
        tree.as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == run.child)
            .unwrap()["name"],
        "rename after exit"
    );
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    assert_eq!(history[0]["outcome"]["status"], "completed");
}

#[tokio::test]
async fn b6_existing_resize_receipts_migrate_without_changing_ids_or_outcomes() {
    let dir = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let project_data = rupd::data::project_data(data.path(), dir.path()).unwrap();
    std::fs::create_dir_all(&project_data).unwrap();
    let db = rusqlite::Connection::open(project_data.join("project.db")).unwrap();
    for schema in [
        include_str!("../migrations/001/up.sql"),
        include_str!("../../agents/migrations/003/up.sql"),
        include_str!("../migrations/003/up.sql"),
        include_str!("../../agents/migrations/004/up.sql"),
    ] {
        db.execute_batch(schema).unwrap();
    }
    db.pragma_update(None, "user_version", 4).unwrap();
    db.execute("INSERT INTO resize_actions(id,actor,agent,attempt,child_terminal,terminal,cols,rows) VALUES(42,?,'old-child','1','old-terminal','old-terminal',101,37)",[json!({"kind":"agent","id":"old-door","parent":null}).to_string()]).unwrap();
    for outcome in [
        json!({"status":"waiting","reason":"takeover"}),
        json!({"status":"in-progress"}),
        json!({"status":"completed"}),
    ] {
        db.execute(
            "INSERT INTO resize_outcomes(action,outcome) VALUES(42,?)",
            [outcome.to_string()],
        )
        .unwrap();
    }
    drop(db);
    let daemon = launch(dir.path(), data.path());
    let user = Client::connect(&daemon.socket).await.unwrap();
    let history = user
        .request("terminal.resizeHistory", json!({"id":"old-terminal"}))
        .await
        .unwrap();
    assert_eq!(
        history,
        json!([{"id":"42","agent":"old-child","terminal":"old-terminal","cols":101,"rows":37,"outcome":{"status":"completed"}}])
    );
    let actions = user
        .request("agent.actionHistory", json!({"id":"old-child"}))
        .await
        .unwrap();
    assert_eq!(
        actions,
        json!([{"id":"42","agent":"old-child","method":"terminal.resize","params":{"id":"old-terminal","cols":101,"rows":37},"outcome":{"status":"completed"}}])
    );
}

#[tokio::test]
async fn b6_held_action_history_is_private_to_the_accepting_agent_and_user() {
    let (run, caller) = door_terminal().await;
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let receipt = caller
        .request(
            "rail.rename",
            json!({"id":run.child,"name":"private rename"}),
        )
        .await
        .unwrap();
    let other = run
        .user
        .request("rail.createWorkstream", json!({"name":"other"}))
        .await
        .unwrap();
    let outsider = Client::connect(&run.daemon.socket).await.unwrap();
    outsider
        .request(
            "daemon.identify",
            json!({"actor":{"kind":"agent","id":other["id"]}}),
        )
        .await
        .unwrap();
    assert_eq!(
        outsider
            .request("agent.actionHistory", json!({"id":run.child}))
            .await
            .unwrap(),
        json!([])
    );
    let forbidden = outsider
        .request(
            "rail.rename",
            json!({"id":run.child,"name":"outside Reach"}),
        )
        .await
        .unwrap_err();
    assert_eq!(forbidden.code, rpc::code::FORBIDDEN);

    assert_eq!(
        caller
            .request("agent.actionHistory", json!({"id":run.child}))
            .await
            .unwrap(),
        json!([receipt.clone()])
    );
    assert_eq!(
        run.user
            .request("agent.actionHistory", json!({"id":run.child}))
            .await
            .unwrap(),
        json!([receipt])
    );
    run.finish().await;
}

#[tokio::test]
async fn b6_a_child_cannot_queue_a_parent_rename_during_the_parents_takeover() {
    let (run, _) = door_terminal().await;
    let (door, _) = start_answer_door(&run).await;
    let child = Client::connect(&run.daemon.socket).await.unwrap();
    child
        .request(
            "daemon.identify",
            json!({"actor":{"kind":"agent","id":run.child}}),
        )
        .await
        .unwrap();
    let params = json!({"id":door,"name":"forbidden parent rename"});
    let before = child
        .request("rail.rename", params.clone())
        .await
        .unwrap_err();
    assert_eq!(before.code, rpc::code::FORBIDDEN);
    run.user
        .request("takeover.begin", json!({"agent":door}))
        .await
        .unwrap();
    let during = child.request("rail.rename", params).await;
    let history = run
        .user
        .request("agent.actionHistory", json!({"id":door}))
        .await
        .unwrap();
    run.user
        .request("takeover.end", json!({"agent":door}))
        .await
        .unwrap();
    run.finish().await;
    assert_eq!(
        during.unwrap_err(),
        before,
        "Takeover must not expand Reach"
    );
    assert_eq!(
        history,
        json!([]),
        "a forbidden action must never be accepted"
    );
}

async fn door_peer_request(run: &Run, terminal: &Value, request: Value) -> Value {
    let result = run.dir.path().join("peer-result");
    if result.exists() {
        std::fs::remove_file(&result).unwrap();
    }
    std::fs::write(
        run.dir.path().join("peer-request"),
        serde_json::to_vec(&request).unwrap(),
    )
    .unwrap();
    run.user
        .request(
            "terminal.write",
            json!({"id":terminal,"data":STANDARD.encode(b"\x02")}),
        )
        .await
        .unwrap();
    // Fake Claude writes this only after reading the real Daemon's RPC replies.
    tokio::time::timeout(BOUND, async {
        loop {
            if let Ok(text) = std::fs::read_to_string(&result)
                && let Ok(reply) = serde_json::from_str(&text)
            {
                return reply;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn b6_a_door_process_cannot_claim_user_to_bypass_a_held_rename() {
    let (run, _) = door_terminal().await;
    let (door, terminal) = start_answer_door(&run).await;
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let replies = door_peer_request(
        &run,
        &terminal,
        json!([
            {"id":1,"method":"daemon.identify","params":{"actor":{"kind":"user","id":"you"}}},
            {"id":2,"method":"rail.rename","params":{"id":run.child,"name":"process rename"}}
        ]),
    )
    .await;
    let reply = replies
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == 2)
        .unwrap()
        .clone();
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    let before = tree
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == run.child)
        .unwrap()["name"]
        .clone();
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    let after = tree
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == run.child)
        .unwrap()["name"]
        .clone();
    run.user
        .request("agent.stop", json!({"id":door}))
        .await
        .unwrap();
    run.finish().await;
    assert_eq!(
        reply["result"]["outcome"],
        json!({"status":"waiting","reason":"takeover"}),
        "{reply}"
    );
    assert_ne!(before, "process rename");
    assert_eq!(after, "process rename");
}

#[tokio::test]
async fn b6_action_history_uses_the_door_process_identity_despite_a_user_claim() {
    let (run, caller) = door_terminal().await;
    let (door, terminal) = start_answer_door(&run).await;
    let child = Client::connect(&run.daemon.socket).await.unwrap();
    child
        .request(
            "daemon.identify",
            json!({"actor":{"kind":"agent","id":run.child}}),
        )
        .await
        .unwrap();
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    child
        .request(
            "rail.rename",
            json!({"id":run.child,"name":"childs rename"}),
        )
        .await
        .unwrap();
    let own = caller
        .request("rail.rename", json!({"id":run.child,"name":"doors rename"}))
        .await
        .unwrap();
    let replies = door_peer_request(
        &run,
        &terminal,
        json!([
            {"id":1,"method":"daemon.identify","params":{"actor":{"kind":"user","id":"you"}}},
            {"id":2,"method":"agent.actionHistory","params":{"id":run.child}}
        ]),
    )
    .await;
    let reply = replies
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == 2)
        .unwrap()
        .clone();
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    run.user
        .request("agent.stop", json!({"id":door}))
        .await
        .unwrap();
    run.finish().await;
    assert_eq!(
        reply["result"],
        json!([own]),
        "the Door read another Actor's receipt: {reply}"
    );
}

#[tokio::test]
async fn b6_a_door_stop_waits_for_takeover_release() {
    let (run, caller) = order_child().await;
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let receipt = caller
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    let terminals = run.user.request("terminal.list", ()).await.unwrap();
    let still_running = terminals
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["id"] == run.terminal && t["running"] == true);
    assert!(
        still_running,
        "Door Stop affected the child during Takeover"
    );
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    let after = run.user.request("terminal.list", ()).await.unwrap();
    let stopped = !after
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["id"] == run.terminal && t["running"] == true);
    run.finish().await;
    assert_eq!(
        receipt["outcome"],
        json!({"status":"waiting","reason":"takeover"})
    );
    assert_eq!(receipt["method"], "agent.stop");
    assert_eq!(history[0]["id"], receipt["id"]);
    assert_eq!(history[0]["outcome"]["status"], "completed");
    assert!(stopped, "released Stop did not end the original Terminal");
}

#[tokio::test]
async fn b6_held_stops_share_resize_and_rename_release_order() {
    let (run, caller) = door_terminal().await;
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    let primary = tree
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == run.child)
        .unwrap()["terminal_id"]
        .clone();
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    for (method, params) in [
        (
            "terminal.resize",
            json!({"id":run.terminal,"cols":103,"rows":39}),
        ),
        ("agent.stop", json!({"id":run.child})),
        ("rail.rename", json!({"id":run.child,"name":"after Stop"})),
        (
            "terminal.resize",
            json!({"id":run.terminal,"cols":104,"rows":40}),
        ),
    ] {
        let receipt = caller.request(method, params).await.unwrap();
        assert_eq!(receipt["outcome"]["status"], "waiting");
    }
    std::fs::write(run.dir.path().join("effect-gate"), "after-resize").unwrap();
    let user = Arc::clone(&run.user);
    let child = run.child.clone();
    let end =
        tokio::spawn(async move { user.request("takeover.end", json!({"agent":child})).await });
    run.paused().await;
    let terminals = run.user.request("terminal.list", ()).await.unwrap();
    assert!(
        terminals
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == primary && t["running"] == true)
    );
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    assert_ne!(
        tree.as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == run.child)
            .unwrap()["name"],
        "after Stop"
    );
    run.release();
    end.await.unwrap().unwrap();
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    assert_eq!(
        history
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["outcome"]["status"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["completed", "completed", "completed", "dropped"]
    );
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    assert_eq!(
        tree.as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == run.child)
            .unwrap()["name"],
        "after Stop"
    );
    assert!(
        history[3]["outcome"]["reason"]
            .as_str()
            .unwrap()
            .contains("exited")
    );
    // Even a stale end must not replay Stop: its Rail Touch is an observable effect.
    let _ = run
        .user
        .request("takeover.end", json!({"agent":run.child}))
        .await;
    let touches = run
        .user
        .request(
            "provenance.history",
            json!({"item":format!("rail:{}",run.child)}),
        )
        .await
        .unwrap();
    let writes = touches
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["actor"]["kind"] == "agent" && t["verb"] == "wrote")
        .count();
    assert_eq!(writes, 2, "exactly one Stop and one rename: {touches}");
    run.finish().await;
}

#[tokio::test]
async fn b6_a_door_process_cannot_claim_user_to_bypass_a_held_stop() {
    let (run, _) = order_child().await;
    let (door, terminal) = start_answer_door(&run).await;
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let replies = door_peer_request(
        &run,
        &terminal,
        json!([
            {"id":1,"method":"daemon.identify","params":{"actor":{"kind":"user","id":"you"}}},
            {"id":2,"method":"agent.stop","params":{"id":run.child}}
        ]),
    )
    .await;
    let reply = replies
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == 2)
        .unwrap();
    assert_eq!(
        reply["result"]["outcome"],
        json!({"status":"waiting","reason":"takeover"}),
        "{replies}"
    );
    let terminals = run.user.request("terminal.list", ()).await.unwrap();
    assert!(
        terminals
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == run.terminal && t["running"] == true)
    );
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let history = run
        .user
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    assert_eq!(history[0]["outcome"]["status"], "completed");
    run.user
        .request("agent.stop", json!({"id":door}))
        .await
        .unwrap();
    run.finish().await;
}

#[tokio::test]
async fn b6_stop_crashes_explain_waiting_and_uncertain_effects_without_repeating() {
    for boundary in ["waiting", "before", "after-stop", "completed"] {
        let (mut run, caller) = order_child().await;
        let tree = run.user.request("rail.tree", ()).await.unwrap();
        let door = tree
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == run.child)
            .unwrap()["parent"]
            .clone();
        run.user
            .request("takeover.begin", json!({"agent":run.child}))
            .await
            .unwrap();
        let receipt = caller
            .request("agent.stop", json!({"id":run.child}))
            .await
            .unwrap();
        if boundary != "waiting" {
            if boundary != "completed" {
                std::fs::write(run.dir.path().join("effect-gate"), boundary).unwrap();
            }
            let user = Arc::clone(&run.user);
            let child = run.child.clone();
            let end =
                tokio::spawn(
                    async move { user.request("takeover.end", json!({"agent":child})).await },
                );
            if boundary == "completed" {
                end.await.unwrap().unwrap();
            } else {
                run.paused().await;
                run.daemon.child.kill().unwrap();
                run.daemon.child.wait().unwrap();
                assert!(end.await.unwrap().is_err());
            }
        }
        if run.daemon.child.try_wait().unwrap().is_none() {
            run.daemon.child.kill().unwrap();
            run.daemon.child.wait().unwrap();
        }
        let _ = std::fs::remove_file(run.dir.path().join("effect-gate"));
        run.daemon = launch(run.dir.path(), run.data.path());
        run.user = Arc::new(Client::connect(&run.daemon.socket).await.unwrap());
        let history = run
            .user
            .request("agent.actionHistory", json!({"id":run.child}))
            .await
            .unwrap();
        assert_eq!(history.as_array().unwrap().len(), 1, "{boundary}");
        assert_eq!(history[0]["id"], receipt["id"]);
        match boundary {
            "completed" => assert_eq!(history[0]["outcome"]["status"], "completed"),
            _ => {
                assert_eq!(history[0]["outcome"]["status"], "dropped");
                let reason = history[0]["outcome"]["reason"].as_str().unwrap();
                assert!(
                    reason.contains(if boundary == "waiting" {
                        "did not survive"
                    } else {
                        "may have happened"
                    }),
                    "{boundary}: {reason}"
                );
            }
        }
        let _ = run
            .user
            .request("takeover.end", json!({"agent":run.child}))
            .await;
        let touches = run
            .user
            .request(
                "provenance.history",
                json!({"item":format!("rail:{}",run.child)}),
            )
            .await
            .unwrap();
        let effects = touches
            .as_array()
            .unwrap()
            .iter()
            .filter(|t| t["actor"]["id"] == door && t["verb"] == "wrote")
            .count();
        assert_eq!(
            effects,
            usize::from(matches!(boundary, "after-stop" | "completed")),
            "{boundary}: {touches}"
        );
        run.finish().await;
    }
}

#[tokio::test]
async fn b6_a_held_stop_never_stops_a_replacement_attempt() {
    for kind in ["user", "ext"] {
        let (mut run, caller) = order_child().await;
        let direct = Arc::new(Client::connect(&run.daemon.socket).await.unwrap());
        direct
            .request(
                "daemon.identify",
                json!({"actor":{"kind":kind,"id":"direct-observer"}}),
            )
            .await
            .unwrap();
        run.user
            .request("takeover.begin", json!({"agent":run.child}))
            .await
            .unwrap();
        caller
            .request("agent.stop", json!({"id":run.child}))
            .await
            .unwrap();
        run.pause();
        let user = Arc::clone(&run.user);
        let child = run.child.clone();
        let end =
            tokio::spawn(async move { user.request("takeover.end", json!({"agent":child})).await });
        run.paused().await;
        tokio::time::timeout(
            Duration::from_secs(2),
            direct.request("agent.stop", json!({"id":run.child})),
        )
        .await
        .expect("direct Stop remains available")
        .unwrap();
        let resumed = direct
            .request("agent.resume", json!({"id":run.child}))
            .await
            .unwrap();
        assert_ne!(resumed["attempt"], run.attempt);
        run.terminal = resumed["terminal_id"].clone();
        run.release();
        end.await.unwrap().unwrap();
        let history = caller
            .request("agent.actionHistory", json!({"id":run.child}))
            .await
            .unwrap();
        let terminals = run.user.request("terminal.list", ()).await.unwrap();
        let replacement_running = terminals
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == run.terminal && t["running"] == true);
        run.finish().await;
        assert!(
            replacement_running,
            "{kind}: held Stop killed the replacement Attempt"
        );
        assert_eq!(history[0]["outcome"]["status"], "dropped");
        assert!(
            history[0]["outcome"]["reason"]
                .as_str()
                .unwrap()
                .contains("original Attempt"),
            "{history}"
        );
    }
}

#[tokio::test]
async fn b6_a_stop_accepted_before_terminal_attachment_keeps_the_same_attempt() {
    let mut run = Run::start().await;
    let fake = run.dir.path().join("claude");
    let source = std::fs::read_to_string(&fake).unwrap();
    std::fs::write(
        &fake,
        source.replace(
            "if sys.argv[1:3] == ['plugin', 'test']:\n    print('Q11 runtime hook closure: 1 pass'); sys.exit(0)",
            r#"if sys.argv[1:3] == ['plugin', 'test']:
    gate = os.path.join(os.path.dirname(__file__), 'launch-gate')
    if os.path.isfile(gate):
        open(gate + '.arrived', 'w').write('plugin test requested')
        deadline = time.monotonic() + 10
        while os.path.isfile(gate):
            if time.monotonic() > deadline: raise RuntimeError('launch probe was not released')
            time.sleep(0.005)
    print('Q11 runtime hook closure: 1 pass'); sys.exit(0)"#,
        ),
    )
    .unwrap();
    // Reopen with an empty runtime admission cache; the probe precedes Terminal creation.
    run.daemon.child.kill().unwrap();
    run.daemon.child.wait().unwrap();
    run.daemon = launch(run.dir.path(), run.data.path());
    run.user = Arc::new(Client::connect(&run.daemon.socket).await.unwrap());
    let door = run
        .user
        .request("rail.createWorkstream", json!({"name":"starting child"}))
        .await
        .unwrap();
    let caller = Client::connect(&run.daemon.socket).await.unwrap();
    caller
        .request(
            "daemon.identify",
            json!({"actor":{"kind":"agent","id":door["id"]}}),
        )
        .await
        .unwrap();
    let gate = run.dir.path().join("launch-gate");
    std::fs::write(&gate, "pause").unwrap();
    let user = Arc::clone(&run.user);
    let cwd = run.dir.path().to_owned();
    let parent = door["id"].clone();
    let spawning = tokio::spawn(async move {
        user.request("agent.spawn", json!({"cwd":cwd,"parent":parent}))
            .await
    });
    tokio::time::timeout(BOUND, async {
        while !gate.with_extension("arrived").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    let starting = tree
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["parent"] == door["id"])
        .unwrap();
    run.child = starting["id"].as_str().unwrap().to_owned();
    assert!(starting["terminal_id"].is_null());
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let receipt = caller
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    assert_eq!(receipt["outcome"]["status"], "waiting");
    std::fs::remove_file(gate).unwrap();
    let attached = spawning.await.unwrap().unwrap();
    assert_eq!(attached["attempt"], starting["attempt"]);
    assert!(attached["terminal_id"].is_string());
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    run.finish().await;
    assert_eq!(history[0]["outcome"]["status"], "completed", "{history}");
}

#[tokio::test]
async fn b6_a_stop_accepted_during_user_resume_follows_the_resuming_attempt() {
    let (mut run, caller) = order_child().await;
    run.user
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    let fake = run.dir.path().join("claude");
    let script = std::fs::read_to_string(&fake).unwrap().replace(
        "hook('SessionStart',",
        "if '--resume' in sys.argv:\n    open('resume-arrived', 'w').close()\n    deadline = time.monotonic() + 10\n    while os.path.isfile('resume-gate'):\n        if time.monotonic() > deadline: raise RuntimeError('resume observer was not released')\n        time.sleep(0.005)\nhook('SessionStart',",
    );
    std::fs::write(fake, script).unwrap();
    let gate = run.dir.path().join("resume-gate");
    std::fs::write(&gate, "pause").unwrap();
    let user = Arc::clone(&run.user);
    let child = run.child.clone();
    let resume =
        tokio::spawn(async move { user.request("agent.resume", json!({"id":child})).await });
    tokio::time::timeout(BOUND, async {
        while !run.dir.path().join("resume-arrived").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    // Direct user Resume must not hold Takeover acquisition behind SessionStart.
    tokio::time::timeout(
        Duration::from_secs(2),
        run.user
            .request("takeover.begin", json!({"agent":run.child})),
    )
    .await
    .expect("Takeover is available before user Resume acknowledges")
    .unwrap();
    let receipt = caller
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    let waiting = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    std::fs::remove_file(gate).unwrap();
    let attached = resume.await.unwrap().unwrap();
    run.terminal = attached["terminal_id"].clone();
    assert_ne!(attached["attempt"], run.attempt);
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    let terminals = run.user.request("terminal.list", ()).await.unwrap();
    let stopped = !terminals
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["id"] == run.terminal && t["running"] == true);
    run.finish().await;
    assert_eq!(waiting[0]["outcome"]["status"], "waiting", "{waiting}");
    assert_eq!(history[0]["id"], receipt["id"]);
    assert_eq!(history[0]["outcome"]["status"], "completed", "{history}");
    assert!(stopped);
}

#[tokio::test]
async fn b6_a_failed_launch_accounts_for_a_held_stop_without_terminal_exit() {
    let mut run = Run::start().await;
    let fake = run.dir.path().join("claude");
    let source = std::fs::read_to_string(&fake).unwrap();
    std::fs::write(
        &fake,
        source.replace(
            "if sys.argv[1:3] == ['plugin', 'test']:\n    print('Q11 runtime hook closure: 1 pass'); sys.exit(0)",
            r#"if sys.argv[1:3] == ['plugin', 'test']:
    gate = os.path.join(os.path.dirname(__file__), 'launch-gate')
    if os.path.isfile(gate):
        open(gate + '.arrived', 'w').write('plugin test requested')
        deadline = time.monotonic() + 10
        while os.path.isfile(gate):
            if time.monotonic() > deadline: raise RuntimeError('launch probe was not released')
            time.sleep(0.005)
    print('unsupported runtime'); sys.exit(1)"#,
        ),
    )
    .unwrap();
    // Reopen with an empty runtime admission cache; the probe precedes Terminal creation.
    run.daemon.child.kill().unwrap();
    run.daemon.child.wait().unwrap();
    run.daemon = launch(run.dir.path(), run.data.path());
    run.user = Arc::new(Client::connect(&run.daemon.socket).await.unwrap());
    let door = run
        .user
        .request("rail.createWorkstream", json!({"name":"starting child"}))
        .await
        .unwrap();
    let caller = Client::connect(&run.daemon.socket).await.unwrap();
    caller
        .request(
            "daemon.identify",
            json!({"actor":{"kind":"agent","id":door["id"]}}),
        )
        .await
        .unwrap();
    let gate = run.dir.path().join("launch-gate");
    std::fs::write(&gate, "pause").unwrap();
    let user = Arc::clone(&run.user);
    let cwd = run.dir.path().to_owned();
    let parent = door["id"].clone();
    let spawning = tokio::spawn(async move {
        user.request("agent.spawn", json!({"cwd":cwd,"parent":parent}))
            .await
    });
    tokio::time::timeout(BOUND, async {
        while !gate.with_extension("arrived").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    let starting = tree
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["parent"] == door["id"])
        .unwrap();
    run.child = starting["id"].as_str().unwrap().to_owned();
    assert!(starting["terminal_id"].is_null());
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let receipt = caller
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    assert_eq!(receipt["outcome"]["status"], "waiting");
    std::fs::remove_file(gate).unwrap();
    assert!(spawning.await.unwrap().is_err());
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    assert_eq!(history[0]["id"], receipt["id"]);
    assert_eq!(history[0]["outcome"]["status"], "dropped", "{history}");
    assert!(
        history[0]["outcome"]["reason"]
            .as_str()
            .unwrap()
            .contains("removed"),
        "{history}"
    );
}

async fn working_child_with_held_interrupt() -> (Run, Arc<Client>, Value) {
    let (run, caller) = order_child().await;
    idle(&run).await;
    run.user.request("agent.signal", json!({"id":run.child,"attempt":run.attempt,"payload":{"hook_event_name":"UserPromptSubmit","prompt":"working"}})).await.unwrap();
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let receipt = caller
        .request("agent.interrupt", json!({"id":run.child}))
        .await
        .unwrap();
    (run, caller, receipt)
}

#[tokio::test]
async fn b6_a_door_interrupt_waits_for_takeover_release() {
    let (run, caller, receipt) = working_child_with_held_interrupt().await;
    assert_eq!(receipt["method"], "agent.interrupt");
    assert_eq!(
        receipt["outcome"],
        json!({"status":"waiting","reason":"takeover"})
    );
    assert!(
        !recorded_bytes(&run).await.contains(&0x1b),
        "Door Interrupt wrote its Escape during Takeover"
    );
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    let escapes = recorded_bytes(&run)
        .await
        .iter()
        .filter(|byte| **byte == 0x1b)
        .count();
    run.finish().await;
    assert_eq!(history[0]["id"], receipt["id"]);
    // The fake Claude never changes its title, so the released Escape's outcome is an
    // unacknowledged one; the receipt records it and the Escape went out exactly once.
    assert_ne!(history[0]["outcome"]["status"], "waiting", "{history}");
    assert_eq!(escapes, 1);
}

#[tokio::test]
async fn b6_a_held_interrupt_is_dropped_when_the_child_exits() {
    let (run, caller, receipt) = working_child_with_held_interrupt().await;
    run.user
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    run.finish().await;
    assert_eq!(history[0]["id"], receipt["id"]);
    assert_eq!(history[0]["outcome"]["status"], "dropped", "{history}");
}

async fn idle_child_with_held_prompt() -> (Run, Arc<Client>, Value) {
    let (run, caller) = order_child().await;
    idle(&run).await;
    std::fs::write(run.dir.path().join("order-submit"), "accept").unwrap();
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let receipt = caller
        .request("agent.prompt", json!({"id":run.child,"text":"held words"}))
        .await
        .unwrap();
    (run, caller, receipt)
}

#[tokio::test]
async fn b6_a_door_prompt_waits_for_takeover_release() {
    let (run, caller, receipt) = idle_child_with_held_prompt().await;
    assert_eq!(receipt["method"], "agent.prompt");
    assert_eq!(
        receipt["outcome"],
        json!({"status":"waiting","reason":"takeover"})
    );
    assert!(
        !run.recorded_input().await.contains("BYTE:"),
        "Door prompt reached the child during Takeover"
    );
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    let typed = String::from_utf8_lossy(&recorded_bytes(&run).await)
        .matches("held words")
        .count();
    run.finish().await;
    assert_eq!(history[0]["id"], receipt["id"]);
    assert_eq!(history[0]["outcome"]["status"], "completed", "{history}");
    assert_eq!(typed, 1);
}

#[tokio::test]
async fn b6_a_held_prompt_is_dropped_when_the_child_exits() {
    let (run, caller, receipt) = idle_child_with_held_prompt().await;
    run.user
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    run.finish().await;
    assert_eq!(history[0]["id"], receipt["id"]);
    assert_eq!(history[0]["outcome"]["status"], "dropped", "{history}");
}

async fn ordered_ask(run: &Run) -> Value {
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    tree.as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == run.child)
        .map(|node| node["work"]["ask"].clone())
        .unwrap()
}

async fn idle_child_with_held_order() -> (Run, Arc<Client>, Value) {
    let (run, caller) = order_child().await;
    idle(&run).await;
    std::fs::write(run.dir.path().join("order-submit"), "accept").unwrap();
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let receipt = order_result(&run, Arc::clone(&caller))
        .await
        .await
        .unwrap()
        .unwrap();
    (run, caller, receipt)
}

#[tokio::test]
async fn b6_a_door_order_waits_for_takeover_release() {
    let (run, caller, receipt) = idle_child_with_held_order().await;
    assert_eq!(receipt["method"], "agent.setOrder");
    assert_eq!(
        receipt["outcome"],
        json!({"status":"waiting","reason":"takeover"})
    );
    assert_ne!(
        ordered_ask(&run).await,
        "first order",
        "Door Order changed the child during Takeover"
    );
    assert!(!run.recorded_input().await.contains("BYTE:"));
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    let ask = ordered_ask(&run).await;
    let typed = String::from_utf8_lossy(&recorded_bytes(&run).await)
        .matches("first order")
        .count();
    run.finish().await;
    assert_eq!(history[0]["id"], receipt["id"]);
    assert_eq!(history[0]["outcome"]["status"], "completed", "{history}");
    assert_eq!(ask, "first order");
    assert_eq!(typed, 1);
}

#[tokio::test]
async fn b6_a_waiting_order_is_dropped_with_a_reason_after_a_daemon_restart() {
    let (mut run, caller, receipt) = idle_child_with_held_order().await;
    run.daemon.child.kill().unwrap();
    run.daemon.child.wait().unwrap();
    run.daemon = launch(run.dir.path(), run.data.path());
    run.user = Arc::new(Client::connect(&run.daemon.socket).await.unwrap());
    // The Attempt does not survive the restart, so the Order is explained, never applied.
    let ask = ordered_ask(&run).await;
    let history = run
        .user
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    drop(caller);
    run.finish().await;
    assert_eq!(history.as_array().unwrap().len(), 1, "{history}");
    assert_eq!(history[0]["id"], receipt["id"]);
    assert_eq!(history[0]["outcome"]["status"], "dropped", "{history}");
    assert!(
        !history[0]["outcome"]["reason"].as_str().unwrap().is_empty(),
        "{history}"
    );
    assert_ne!(ask, "first order");
}

#[tokio::test]
async fn b6_a_held_order_is_dropped_when_the_child_exits() {
    let (run, caller, receipt) = idle_child_with_held_order().await;
    run.user
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    run.finish().await;
    assert_eq!(history[0]["id"], receipt["id"]);
    assert_eq!(history[0]["outcome"]["status"], "dropped", "{history}");
}

async fn child_attempt(run: &Run) -> Value {
    let tree = run.user.request("rail.tree", ()).await.unwrap();
    tree.as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == run.child)
        .map(|node| node["attempt"].clone())
        .unwrap()
}

async fn idle_child_with_held_resume() -> (Run, Arc<Client>, Value) {
    let (run, caller) = order_child().await;
    idle(&run).await;
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let receipt = caller
        .request("agent.resume", json!({"id":run.child}))
        .await
        .unwrap();
    (run, caller, receipt)
}

#[tokio::test]
async fn b6_a_door_resume_waits_for_takeover_release() {
    let (run, caller, receipt) = idle_child_with_held_resume().await;
    assert_eq!(receipt["method"], "agent.resume");
    assert_eq!(
        receipt["outcome"],
        json!({"status":"waiting","reason":"takeover"})
    );
    assert_eq!(child_attempt(&run).await, run.attempt);
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    let attempt = child_attempt(&run).await.clone();
    let original = run.attempt.clone();
    run.finish().await;
    assert_eq!(history[0]["id"], receipt["id"]);
    // The child never exited, so there is nothing to resume: why is recorded.
    assert_eq!(history[0]["outcome"]["status"], "dropped", "{history}");
    assert_eq!(attempt, original);
}

#[tokio::test]
async fn b6_a_held_resume_runs_once_when_the_child_exits_during_takeover() {
    let (run, caller, receipt) = idle_child_with_held_resume().await;
    run.user
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    let history = tokio::time::timeout(BOUND, async {
        loop {
            let history = caller
                .request("agent.actionHistory", json!({"id":run.child}))
                .await
                .unwrap();
            if history[0]["outcome"]["status"] != "in-progress" {
                break history;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the held Resume never finished");
    let attempt = child_attempt(&run).await.clone();
    let original = run.attempt.clone();
    run.finish().await;
    assert_eq!(history.as_array().unwrap().len(), 1, "{history}");
    assert_eq!(history[0]["id"], receipt["id"]);
    assert_eq!(history[0]["outcome"]["status"], "completed", "{history}");
    assert_ne!(attempt, original, "the held Resume started no new Attempt");
}

async fn held_door_answer(
    run: &Run,
    kind: &str,
) -> (
    Value,
    tokio::task::JoinHandle<Result<Value, RpcError>>,
    Value,
) {
    let (_, terminal) = start_answer_door(run).await;
    let (decision, waiting) = run.open(kind).await;
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let reply = door_peer_request(
        run,
        &terminal,
        json!({"jsonrpc":"2.0","id":1,"method":"decision.answer",
            "params":{"id":decision["id"],"answer":if kind == "permission" {"allow"} else {"Yes"}}}),
    )
    .await;
    (decision, waiting, reply["result"].clone())
}

#[tokio::test]
async fn u468_takeover_holds_door_answers_but_not_user_answers() {
    for kind in ["permission", "ask_user"] {
        let (run, _) = order_child().await;
        let (decision, waiting, receipt) = held_door_answer(&run, kind).await;
        assert_eq!(receipt["method"], "decision.answer", "{receipt}");
        assert_eq!(
            receipt["outcome"],
            json!({"status":"waiting","reason":"takeover"})
        );
        let open = run.user.request("decision.list", ()).await.unwrap();
        assert_eq!(
            open.as_array().unwrap().len(),
            1,
            "answered during Takeover"
        );
        assert!(!waiting.is_finished());
        run.user
            .request("takeover.end", json!({"agent":run.child}))
            .await
            .unwrap();
        let output = waiting.await.unwrap().unwrap();
        let history = run
            .user
            .request("agent.actionHistory", json!({"id":run.child}))
            .await
            .unwrap();
        run.finish().await;
        assert!(
            output
                .to_string()
                .contains(if kind == "permission" { "allow" } else { "Yes" })
        );
        assert_eq!(history[0]["id"], receipt["id"]);
        assert_eq!(history[0]["outcome"]["status"], "completed", "{history}");
        assert_eq!(decision["agent"], history[0]["agent"]);
    }
}

#[tokio::test]
async fn u468_an_answered_request_drops_the_held_door_answer_visibly() {
    let (run, _) = order_child().await;
    let (decision, waiting, receipt) = held_door_answer(&run, "permission").await;
    run.user_answer(&decision, "permission").await;
    waiting.await.unwrap().unwrap();
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let history = run
        .user
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    run.finish().await;
    assert_eq!(history[0]["id"], receipt["id"]);
    assert_eq!(history[0]["outcome"]["status"], "dropped", "{history}");
    assert!(!history[0]["outcome"]["reason"].as_str().unwrap().is_empty());
}

async fn terminal_running(run: &Run) -> bool {
    let listed = run.user.request("terminal.list", json!({})).await.unwrap();
    listed
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["id"] == run.terminal && t["running"] == true)
}

async fn idle_child_with_held_terminal_call(method: &str) -> (Run, Arc<Client>, Value) {
    let (run, caller) = door_terminal().await;
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let receipt = caller
        .request(method, json!({"id":run.terminal}))
        .await
        .unwrap();
    (run, caller, receipt)
}

async fn held_terminal_call_waits_for_release(method: &str) {
    let (run, caller, receipt) = idle_child_with_held_terminal_call(method).await;
    assert_eq!(receipt["method"], method);
    assert_eq!(
        receipt["outcome"],
        json!({"status":"waiting","reason":"takeover"})
    );
    assert!(
        terminal_running(&run).await,
        "Door {method} reached the Terminal during Takeover"
    );
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    let running = terminal_running(&run).await;
    run.finish().await;
    assert_eq!(history[0]["id"], receipt["id"]);
    assert_eq!(history[0]["outcome"]["status"], "completed", "{history}");
    assert!(!running, "released {method} left the Terminal running");
}

#[tokio::test]
async fn b6_a_door_terminal_kill_waits_for_takeover_release() {
    held_terminal_call_waits_for_release("terminal.kill").await;
}

#[tokio::test]
async fn b6_a_door_terminal_close_waits_for_takeover_release() {
    held_terminal_call_waits_for_release("terminal.close").await;
}

#[tokio::test]
async fn b6_a_held_terminal_kill_is_dropped_when_the_child_exits() {
    let (run, caller, receipt) = idle_child_with_held_terminal_call("terminal.kill").await;
    run.user
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    run.finish().await;
    assert_eq!(history[0]["id"], receipt["id"]);
    assert_eq!(history[0]["outcome"]["status"], "dropped", "{history}");
}

#[tokio::test]
async fn b6_a_door_terminal_write_waits_for_takeover_release() {
    let (run, caller) = door_terminal().await;
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let receipt = caller
        .request(
            "terminal.write",
            json!({"id":run.terminal,"data":STANDARD.encode(b"door")}),
        )
        .await
        .unwrap();
    assert_eq!(receipt["method"], "terminal.write");
    assert_eq!(
        receipt["outcome"],
        json!({"status":"waiting","reason":"takeover"})
    );
    // The user's own keystrokes still reach the child; the Door's bytes come after the end.
    run.user
        .request(
            "terminal.write",
            json!({"id":run.terminal,"data":STANDARD.encode(b"user")}),
        )
        .await
        .unwrap();
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    let bytes = tokio::time::timeout(BOUND, async {
        loop {
            let bytes = recorded_bytes(&run).await;
            if bytes.len() >= 8 {
                return bytes;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    run.finish().await;
    assert_eq!(history[0]["id"], receipt["id"]);
    assert_eq!(history[0]["outcome"]["status"], "completed", "{history}");
    assert_eq!(bytes, b"userdoor");
}

#[tokio::test]
async fn b6_a_held_terminal_write_is_dropped_when_the_child_exits() {
    let (run, caller) = door_terminal().await;
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let receipt = caller
        .request(
            "terminal.write",
            json!({"id":run.terminal,"data":STANDARD.encode(b"door")}),
        )
        .await
        .unwrap();
    run.user
        .request("agent.stop", json!({"id":run.child}))
        .await
        .unwrap();
    let history = caller
        .request("agent.actionHistory", json!({"id":run.child}))
        .await
        .unwrap();
    run.finish().await;
    assert_eq!(history[0]["id"], receipt["id"]);
    assert_eq!(history[0]["outcome"]["status"], "dropped", "{history}");
}

#[tokio::test]
async fn b6_a_door_read_of_a_taken_over_child_waits_for_the_takeover_end() {
    let (run, caller) = door_terminal().await;
    run.user
        .request("takeover.begin", json!({"agent":run.child}))
        .await
        .unwrap();
    let terminal = run.terminal.clone();
    let read = tokio::spawn(async move {
        caller
            .request("terminal.snapshot", json!({"id":terminal}))
            .await
    });
    // The user's own read is never held.
    run.user
        .request("terminal.snapshot", json!({"id":run.terminal}))
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(!read.is_finished(), "a Door read ran during Takeover");
    run.user
        .request("takeover.end", json!({"agent":run.child}))
        .await
        .unwrap();
    let snapshot = tokio::time::timeout(BOUND, read).await.unwrap().unwrap();
    run.finish().await;
    assert!(snapshot.unwrap()["cols"].is_number());
}
