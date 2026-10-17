//! B50, B51, B52: a `note` goes through Claude Code's messaging socket and never to the Terminal.
//! A real Daemon, the fake `claude` (it listens on the socket it is launched with, lists its
//! session as Claude Code does, and records every byte written to its Terminal) and the fake
//! transcript the test appends to.
#[path = "support/rupd_harness.rs"]
mod rupd_harness;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use contracts::EventData;
use rpc::Client;
use serde_json::{Value, json};

const BOUND: Duration = Duration::from_secs(15);

const FAKE: &str = r#"#!/usr/bin/env python3
import json, os, re, socket, subprocess, sys, threading, tty
here = os.path.dirname(os.path.abspath(__file__))
if sys.argv[1:2] == ['--version']:
    print(open(here + '/version').read().strip() if os.path.isfile(here + '/version') else '2.1.300 (Claude Code)'); sys.exit(0)
if sys.argv[1:3] == ['plugin', 'list']:
    print('[]'); sys.exit(0)
if sys.argv[1:3] == ['plugin', 'test']:
    print('Q11 runtime hook closure: 1 pass'); sys.exit(0)
def flag(name):
    return sys.argv[sys.argv.index(name) + 1]
# A session lists itself with its token, and listens; a line is kept only when it is the one
# Claude Code takes up: past `auth`, and from a sender whose mode is this session's.
path, token = flag('--messaging-socket-path'), 'peer-token-' + str(os.getpid())
sessions = os.environ['CLAUDE_CONFIG_DIR'] + '/sessions'
os.makedirs(sessions, exist_ok=True)
server = socket.socket(socket.AF_UNIX)
server.bind(path)
server.listen()
json.dump({'pid': os.getpid(), 'messagingSocketPath': path}, open(sessions + '/%d.json' % os.getpid(), 'w'))
json.dump({'peerToken': token}, open(sessions + '/%d.abc.key' % os.getpid(), 'w'))
def take(conn):
    lines = conn.makefile()
    if json.loads(lines.readline()).get('token') != token:
        return
    for line in lines:
        mode = re.search(r'from-mode="([^"]*)"', json.loads(line)['message']['content']).group(1)
        if mode == flag('--permission-mode'):
            with open(here + '/received.jsonl', 'a') as out: out.write(line)
def serve():
    while True:
        conn, _ = server.accept()
        threading.Thread(target=take, args=(conn,), daemon=True).start()
open(here + '/received.jsonl', 'w').close()
threading.Thread(target=serve, daemon=True).start()
tty.setraw(0)
print('\033]0;✳ Ready\007', flush=True)
settings = json.load(open(sys.argv[sys.argv.index('--settings') + 1]))
def hook(event, fields):
    fields['hook_event_name'] = event
    for group in settings['hooks'][event]:
        for item in group['hooks']:
            subprocess.run(item['command'], shell=True, input=json.dumps(fields), text=True, check=True)
hook('SessionStart', {'session_id': '00000000-0000-4000-8000-000000000050', 'source': 'resume' if '--resume' in sys.argv else 'startup', 'transcript_path': here + '/transcript.jsonl'})
hook('Stop', {})
print('READY', flush=True)
line = b''
while True:
    data = os.read(0, 1)
    if not data: break
    # The Terminal read caps at 200 lines; the bytes typed are counted from this file instead.
    with open(here + '/typed.bin', 'ab') as typed: typed.write(data)
    if data == b'\r':
        # A submitted prompt is signalled as Claude Code's `UserPromptSubmit`, then its turn ends.
        text = line.replace(b'\x1b[200~', b'').replace(b'\x1b[201~', b'').decode(errors='replace').strip()
        line = b''
        hook('UserPromptSubmit', {'prompt': text, 'transcript_path': here + '/transcript.jsonl'})
        hook('Stop', {'transcript_path': here + '/transcript.jsonl'})
    else:
        line += data
"#;

struct Run {
    daemon: rupd_harness::Rupd,
    dir: tempfile::TempDir,
    data: tempfile::TempDir,
    user: Client,
    watcher: Client,
    sender: Client,
    child: String,
    terminal: Value,
    attempt: Value,
    /// The bytes the Daemon typed itself at the start (its context), before any note.
    started_with: AtomicUsize,
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
                .env("ROUNDUP_DATA_DIR", data)
                .env("ROUNDUP_CLAUDE_BIN", project.join("claude"))
                .env("ROUNDUP_RUP_BIN", rup)
                .env("CLAUDE_CONFIG_DIR", project.join("config"))
                .stderr(std::process::Stdio::null());
        },
    )
}

async fn until<T>(what: &str, mut probe: impl AsyncFnMut() -> Option<T>) -> T {
    tokio::time::timeout(BOUND, async {
        loop {
            if let Some(found) = probe().await {
                return found;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what}"))
}

impl Run {
    /// A Daemon with one idle Agent.
    async fn start() -> Self {
        Self::start_on("2.1.300 (Claude Code)").await
    }

    /// A Daemon whose `claude --version` prints `version`.
    async fn start_on(version: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let fake = dir.path().join("claude");
        std::fs::write(&fake, FAKE).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(dir.path().join("version"), version).unwrap();
        let daemon = launch(dir.path(), data.path());
        let user = Client::connect(&daemon.socket).await.unwrap();
        let mut watcher = Client::connect(&daemon.socket).await.unwrap();
        watcher
            .request("events.subscribe", Value::Null)
            .await
            .unwrap();
        let _ = &mut watcher;
        let sender = Client::connect(&daemon.socket).await.unwrap();
        sender
            .request(
                "daemon.identify",
                json!({"actor":{"kind":"ext","id":"b50"}}),
            )
            .await
            .unwrap();
        let mut run = Self {
            daemon,
            dir,
            data,
            user,
            watcher,
            sender,
            child: String::new(),
            terminal: Value::Null,
            attempt: Value::Null,
            started_with: AtomicUsize::new(0),
        };
        run.spawn().await;
        run
    }

    async fn spawn(&mut self) {
        let node = self
            .user
            .request("agent.spawn", json!({"name":"child","cwd":self.dir.path()}))
            .await
            .unwrap();
        self.child = node["id"].as_str().unwrap().to_owned();
        self.terminal = node["terminal_id"].clone();
        self.attempt = node["attempt"].clone();
        self.ready().await;
    }

    async fn ready(&self) {
        until("the fake claude is ready", async || {
            self.screen().await.contains("READY").then_some(())
        })
        .await;
        // The Daemon types its own start-up context first; count from where that ends.
        let mut last = usize::MAX;
        while last != self.screen_bytes().await {
            last = self.screen_bytes().await;
            tokio::time::sleep(Duration::from_millis(400)).await;
        }
        self.started_with.store(last, Ordering::SeqCst);
    }

    async fn screen_bytes(&self) -> usize {
        std::fs::metadata(self.dir.path().join("typed.bin")).map_or(0, |m| m.len() as usize)
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

    /// The bytes written to the Terminal since the Agent started, as the fake `claude` read them.
    async fn typed(&self) -> usize {
        self.screen_bytes().await - self.started_with.load(Ordering::SeqCst)
    }

    fn sessions(&self) -> PathBuf {
        self.dir.path().join("config/sessions")
    }

    /// Takes the session off the list, as when Claude Code is not listening; the files, to put back.
    fn unlist_session(&self) -> Vec<(PathBuf, Vec<u8>)> {
        let files: Vec<_> = std::fs::read_dir(self.sessions())
            .unwrap()
            .map(|file| {
                let path = file.unwrap().path();
                let bytes = std::fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect();
        for (path, _) in &files {
            std::fs::remove_file(path).unwrap();
        }
        files
    }

    fn relist_session(&self, files: &[(PathBuf, Vec<u8>)]) {
        for (path, bytes) in files {
            std::fs::write(path, bytes).unwrap();
        }
    }

    /// The Messages' lines the fake session took up over its socket, as `rup-<id>`: only a line
    /// whose `from-mode` is the mode it was launched in counts, as the rest are held.
    fn entries(&self) -> Vec<String> {
        let text =
            std::fs::read_to_string(self.dir.path().join("received.jsonl")).unwrap_or_default();
        text.lines()
            .map(|line| {
                let msg_id = serde_json::from_str::<Value>(line).unwrap()["msg_id"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                let id = u64::from_str_radix(msg_id.rsplit('-').next().unwrap(), 16).unwrap();
                format!("rup-{id}")
            })
            .collect()
    }

    /// The Message lines taken up, whole.
    fn received(&self) -> Vec<Value> {
        let text =
            std::fs::read_to_string(self.dir.path().join("received.jsonl")).unwrap_or_default();
        text.lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    async fn note(&self, body: &str) -> u32 {
        let sent = self
            .sender
            .request(
                "message.send",
                json!({"to":self.child,"kind":"note","body":body}),
            )
            .await
            .unwrap();
        sent["id"].as_u64().unwrap() as u32
    }

    /// The Agent works, then is idle again: one `idle` edge.
    async fn idle_edge(&self) {
        for event in ["PreToolUse", "Stop"] {
            self.signal(json!({"hook_event_name":event,"tool_name":"Bash"}))
                .await;
        }
    }

    async fn signal(&self, mut payload: Value) {
        payload["transcript_path"] = json!(self.dir.path().join("transcript.jsonl"));
        self.user
            .request(
                "agent.signal",
                json!({"id":self.child,"attempt":self.attempt,"payload":payload}),
            )
            .await
            .unwrap();
    }

    /// The transcript records the entries of `ids` the way Claude Code does, then the Agent signals.
    async fn record(&self, ids: &[u32]) {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.path().join("transcript.jsonl"))
            .unwrap();
        for id in ids {
            writeln!(
                file,
                "{}",
                json!({"type":"user","origin":{"kind":"peer","from":"uds:/rup/rupd","msg_id":format!("00000000-0000-4000-8000-{id:012x}")}})
            )
            .unwrap();
        }
        self.signal(json!({"hook_event_name":"PostToolUse","tool_name":"Bash"}))
            .await;
    }

    async fn message(&self, id: u32) -> Value {
        self.user
            .request("message.get", json!({"id":id}))
            .await
            .unwrap()
    }

    async fn status(&self, id: u32) -> (String, Option<String>) {
        let message = self.message(id).await;
        (
            message["status"].as_str().unwrap().to_owned(),
            message["reason"].as_str().map(str::to_owned),
        )
    }

    async fn rail_inbox(&self) -> Option<String> {
        let tree = self.user.request("rail.tree", Value::Null).await.unwrap();
        let node = tree
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == self.child.as_str());
        node.unwrap()["inbox"].as_str().map(str::to_owned)
    }

    /// The `message.held` and `message.delivered` ids seen so far, in order.
    async fn seen(&mut self) -> (Vec<u32>, Vec<u32>) {
        let (mut held, mut delivered) = (Vec::new(), Vec::new());
        while let Ok(Some(event)) =
            tokio::time::timeout(Duration::from_millis(300), self.watcher.next_event()).await
        {
            match event.data {
                EventData::MessageHeld(m) => held.push(m.id),
                EventData::MessageDelivered(m) => delivered.push(m.id),
                _ => {}
            }
        }
        (held, delivered)
    }
}

#[tokio::test]
async fn b50_a_note_goes_over_the_socket_labelled_with_its_sender_and_not_held() {
    let run = Run::start().await;
    let id = run.note("look at the build").await;
    // The fake counts a line only when its `from-mode` is the mode the Agent was launched in,
    // so a note it took up was not held.
    until("the note is taken up", async || {
        (run.entries() == [format!("rup-{id}")]).then_some(())
    })
    .await;
    assert_eq!(run.typed().await, 0);
    let content = run.received()[0]["message"]["content"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        content.contains(
            "from-name=\"b50\" from-mode=\"default\">\nlook at the build\n</cross-session-message>"
        ),
        "{content}"
    );
    assert_eq!(run.rail_inbox().await.as_deref(), Some("ok"));
}

#[tokio::test]
async fn b50_a_note_is_delivered_when_the_transcript_records_it() {
    let mut run = Run::start().await;
    let id = run.note("look").await;
    until("taken up", async || {
        (!run.entries().is_empty()).then_some(())
    })
    .await;
    assert_eq!(run.status(id).await, ("pending".into(), None));
    run.idle_edge().await;
    assert_eq!(run.status(id).await.0, "pending", "no idle delivers it");
    run.record(&[id]).await;
    until("delivered", async || {
        (run.status(id).await.0 == "delivered").then_some(())
    })
    .await;
    let (_, delivered) = run.seen().await;
    assert_eq!(delivered, [id]);
}

#[tokio::test]
async fn b50_a_long_note_shows_as_one_line() {
    let run = Run::start().await;
    let body = format!("[wake] {}", "é".repeat(300));
    let id = run.note(&body).await;
    until("taken up", async || {
        (!run.entries().is_empty()).then_some(())
    })
    .await;
    let content = run.received()[0]["message"]["content"]
        .as_str()
        .unwrap()
        .to_owned();
    let text = content.split('\n').nth(1).unwrap();
    assert!(text.len() <= 200 && text.ends_with("[cut]"), "{text}");
    assert_eq!(run.message(id).await["body"], body.as_str());
}

#[tokio::test]
async fn b51_a_session_not_listening_holds_the_note_with_zero_terminal_writes() {
    let mut run = Run::start().await;
    run.unlist_session();
    let id = run.note("hello").await;
    until("held", async || {
        (run.status(id).await.0 == "held").then_some(())
    })
    .await;
    assert_eq!(run.status(id).await, ("held".into(), Some("inbox".into())));
    assert_eq!(run.typed().await, 0);
    assert!(run.entries().is_empty());
    assert_eq!(run.rail_inbox().await.as_deref(), Some("missing"));
    assert_eq!(run.seen().await.0, [id]);
}

#[tokio::test]
async fn b51_an_unprobed_claude_version_is_a_mismatch() {
    let run = Run::start_on("9.9.9 (Claude Code)").await;
    let id = run.note("hello").await;
    until("held", async || {
        (run.status(id).await.0 == "held").then_some(())
    })
    .await;
    assert_eq!(run.rail_inbox().await.as_deref(), Some("mismatch"));
    assert!(run.entries().is_empty());
    assert_eq!(run.typed().await, 0);
}

#[tokio::test]
async fn b51_a_held_inbox_note_is_released_when_the_session_listens_again() {
    let mut run = Run::start().await;
    let listed = run.unlist_session();
    let first = run.note("one").await;
    let second = run.note("two").await;
    until("held", async || {
        (run.status(second).await.0 == "held" && run.status(first).await.0 == "held").then_some(())
    })
    .await;
    assert_eq!(run.rail_inbox().await.as_deref(), Some("missing"));
    run.relist_session(&listed);
    run.idle_edge().await;
    until("taken up", async || {
        (run.entries() == [format!("rup-{first}"), format!("rup-{second}")]).then_some(())
    })
    .await;
    assert_eq!(run.rail_inbox().await.as_deref(), Some("ok"));
    assert_eq!(run.typed().await, 0);
    let _ = run.seen().await;
}

#[tokio::test]
async fn b51_deliver_by_hand_sends_nothing_and_errors() {
    let run = Run::start().await;
    run.unlist_session();
    let id = run.note("hello").await;
    until("held", async || {
        (run.status(id).await.0 == "held").then_some(())
    })
    .await;
    let err = run
        .user
        .request("message.deliver", json!({"id":id}))
        .await
        .unwrap_err();
    assert_eq!(err.code, rpc::code::INBOX_UNAVAILABLE);
    assert_eq!(run.status(id).await.0, "held");
    assert_eq!(run.typed().await, 0);
}

#[tokio::test]
async fn b51_the_user_message_is_typed_whatever_the_inbox() {
    let run = Run::start().await;
    run.unlist_session();
    run.user
        .request(
            "message.send",
            json!({"to":run.child,"kind":"note","body":"from me"}),
        )
        .await
        .unwrap();
    until("typed", async || (run.typed().await > 0).then_some(())).await;
}

#[tokio::test]
async fn b52_thirty_idle_edges_send_a_note_once_and_type_nothing() {
    let mut run = Run::start().await;
    let id = run.note("once").await;
    until("taken up", async || {
        (!run.entries().is_empty()).then_some(())
    })
    .await;
    for _ in 0..30 {
        run.idle_edge().await;
    }
    assert_eq!(run.entries(), [format!("rup-{id}")]);
    assert_eq!(run.typed().await, 0);
    run.record(&[id]).await;
    until("delivered", async || {
        (run.status(id).await.0 == "delivered").then_some(())
    })
    .await;
    assert_eq!(run.seen().await.1, [id]);
}

#[tokio::test]
async fn b52_thirty_idle_edges_with_a_session_not_listening_hold_once_and_type_nothing() {
    let mut run = Run::start().await;
    run.unlist_session();
    let id = run.note("once").await;
    for _ in 0..30 {
        run.idle_edge().await;
    }
    assert_eq!(run.status(id).await, ("held".into(), Some("inbox".into())));
    assert_eq!(run.typed().await, 0);
    assert_eq!(run.seen().await.0, [id]);
}

#[tokio::test]
async fn b52_a_restart_with_a_held_inbox_note_keeps_it_held() {
    let mut run = Run::start_on("9.9.9 (Claude Code)").await;
    let id = run.note("keep").await;
    until("held", async || {
        (run.status(id).await.0 == "held").then_some(())
    })
    .await;
    run.restart().await;
    assert_eq!(run.status(id).await, ("held".into(), Some("inbox".into())));
    assert_eq!(run.typed().await, 0);
}

#[tokio::test]
async fn b52_a_restart_with_a_sent_note_delivers_it_once() {
    let mut run = Run::start().await;
    let id = run.note("once").await;
    until("taken up", async || {
        (!run.entries().is_empty()).then_some(())
    })
    .await;
    run.restart().await;
    run.idle_edge().await;
    // The resumed session is a new process with its own socket; it takes the note up once.
    until("taken up again", async || {
        (run.entries() == [format!("rup-{id}")]).then_some(())
    })
    .await;
    run.record(&[id]).await;
    until("delivered", async || {
        (run.status(id).await.0 == "delivered").then_some(())
    })
    .await;
    assert_eq!(run.entries(), [format!("rup-{id}")]);
    assert_eq!(run.typed().await, 0);
    let _ = run.seen().await;
}

impl Run {
    /// A Daemon kill (D6), then a new Daemon on the same Project, and the Agent resumed.
    async fn restart(&mut self) {
        let socket = self.daemon.socket.clone();
        let _ = self.daemon.child.kill();
        let _ = self.daemon.child.wait();
        let _ = std::fs::remove_file(&socket);
        self.daemon = launch(self.dir.path(), self.data.path());
        self.user = Client::connect(&self.daemon.socket).await.unwrap();
        self.watcher = Client::connect(&self.daemon.socket).await.unwrap();
        self.watcher
            .request("events.subscribe", Value::Null)
            .await
            .unwrap();
        self.sender = Client::connect(&self.daemon.socket).await.unwrap();
        self.sender
            .request(
                "daemon.identify",
                json!({"actor":{"kind":"ext","id":"b50"}}),
            )
            .await
            .unwrap();
        let node = self
            .user
            .request("agent.resume", json!({"id":self.child}))
            .await
            .unwrap();
        self.terminal = node["terminal_id"].clone();
        self.attempt = node["attempt"].clone();
        self.ready().await;
    }
}
