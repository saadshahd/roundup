//! Scenarios S1 to S3 (`scenarios/app.md`). A fake `rupd` is a state script; the real Daemon runs
//! in-process on a socket the script links to the one the App chose.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use desktop::{AppState, Config, build, handle_run_event};
use rpc::code;
use serde_json::{Value, json};
use tauri::ipc::{Channel, InvokeBody, InvokeResponseBody};
use tauri::test::{
    INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
};
use tauri::webview::InvokeRequest;
use tauri::{App, Listener, Manager, RunEvent, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tempfile::TempDir;
use tokio::net::UnixListener;

/// Every wait in these tests is bounded by this; nothing sleeps for a fixed time.
const WAIT: Duration = Duration::from_secs(10);

const BURST: usize = 50;

struct Fixture {
    dir: TempDir,
    project: PathBuf,
    config: Config,
}

impl Fixture {
    /// `body` is the fake `rupd`. `{link}` in it becomes a command that points the App's socket
    /// at the real Daemon; `{dir}` is the fixture's directory; `read go < '{dir}/go'` waits for `release`.
    fn new(body: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("my-project");
        std::fs::create_dir(&project).unwrap();
        let link = format!(
            "ln -s '{}' \"$RUPD_SOCKET\"",
            dir.path().join("real.sock").display()
        );
        let script = dir.path().join("rupd");
        let body = body
            .replace("{link}", &link)
            .replace("{dir}", &dir.path().display().to_string());
        let go = dir.path().join("go");
        assert!(Command::new("mkfifo").arg(&go).status().unwrap().success());
        std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let config = Config::locate(
            Some(script.into_os_string()),
            Path::new("/unused/roundup"),
            dir.path(),
            std::process::id(),
        )
        .unwrap();
        Self {
            dir,
            project,
            config,
        }
    }

    /// Lets a fake `rupd` that is blocked on `read go < '{dir}/go'` continue. Opening a fifo for
    /// writing blocks until a reader opens it, so a fake that died first would hang the test.
    fn release(&self) {
        let go = self.dir.path().join("go");
        let (done, written) = mpsc::channel();
        std::thread::spawn(move || done.send(std::fs::write(go, "go\n")));
        written
            .recv_timeout(WAIT)
            .expect("the fake rupd never read from go")
            .unwrap();
    }

    fn real_daemon_socket(&self) -> PathBuf {
        self.dir.path().join("real.sock")
    }

    fn serve_real_daemon(&self) {
        let daemon = rupd::Daemon::open(&self.dir.path().join(".roundup")).unwrap();
        let socket = self.real_daemon_socket();
        tauri::async_runtime::block_on(async move {
            let listener = UnixListener::bind(socket).unwrap();
            tauri::async_runtime::spawn(rupd::serve(listener, Arc::new(daemon)));
        });
    }
}

fn app(config: Config, folder: Option<PathBuf>) -> (App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let app = build(mock_builder(), config, folder, |app, code| app.exit(code))
        .build(mock_context(noop_assets()))
        .unwrap();
    let webview = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
        .build()
        .unwrap();
    (app, webview)
}

fn real_context_app(config: Config) -> (App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let app = build(mock_builder(), config, None, |app, code| app.exit(code))
        .build(desktop::context())
        .unwrap();
    let webview = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
        .build()
        .unwrap();
    (app, webview)
}

fn event_channel() -> (Channel<contracts::Event>, mpsc::Receiver<Value>) {
    let (tx, received) = mpsc::channel();
    let channel = Channel::new(move |body| {
        let InvokeResponseBody::Json(json) = body else {
            panic!("events are JSON")
        };
        tx.send(serde_json::from_str::<Value>(&json).unwrap())
            .unwrap();
        Ok(())
    });
    (channel, received)
}

fn invoke(webview: &WebviewWindow<MockRuntime>, cmd: &str, args: Value) -> Result<Value, Value> {
    get_ipc_response(
        webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: InvokeBody::Json(args),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|body| body.deserialize().unwrap())
}

fn path_arg(path: &Path) -> Value {
    json!({ "path": path.display().to_string() })
}

fn open(webview: &WebviewWindow<MockRuntime>, project: &Path) -> Value {
    invoke(webview, "open_project", path_arg(project)).unwrap()
}

fn eventually(what: &str, condition: impl Fn() -> bool) {
    let start = Instant::now();
    while !condition() {
        assert!(
            start.elapsed() < WAIT,
            "{what} did not happen within {WAIT:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn daemon_exited(app: &App<MockRuntime>) -> mpsc::Receiver<Value> {
    let (tx, rx) = mpsc::channel();
    app.listen("daemon-exited", move |event| {
        tx.send(serde_json::from_str(event.payload()).unwrap())
            .unwrap();
    });
    rx
}

const SERVE_AND_WAIT: &str = "{link}\nexec cat";

#[test]
fn s1_open_project_starts_rupd_attached_on_the_socket_the_app_chose() {
    let fx = Fixture::new(
        "printf '%s\\n' \"$@\" > '{dir}/args'\nprintf '%s' \"$RUPD_SOCKET\" > '{dir}/socket'\n{link}\nexec cat",
    );
    fx.serve_real_daemon();
    let (_app, webview) = app(fx.config.clone(), None);

    let opened = open(&webview, &fx.project);

    assert_eq!(
        opened,
        json!({ "name": "my-project", "path": fx.project.display().to_string() })
    );
    assert_eq!(
        std::fs::read_to_string(fx.dir.path().join("args")).unwrap(),
        format!("{}\n--attached\n", fx.project.display())
    );
    assert_eq!(
        std::fs::read_to_string(fx.dir.path().join("socket")).unwrap(),
        fx.config.socket.display().to_string()
    );
}

#[test]
fn s1_project_is_null_until_one_is_open_and_then_the_open_one() {
    let fx = Fixture::new(SERVE_AND_WAIT);
    fx.serve_real_daemon();
    let (_app, webview) = app(fx.config.clone(), None);
    assert_eq!(invoke(&webview, "project", json!({})), Ok(Value::Null));

    let opened = open(&webview, &fx.project);

    assert_eq!(invoke(&webview, "project", json!({})), Ok(opened));
}

#[test]
fn s1_the_socket_is_unique_to_the_run_and_under_the_unix_limit() {
    let tmp = Path::new("/tmp");
    let first = Config::locate(None, Path::new("/a/roundup"), tmp, 100).unwrap();
    let second = Config::locate(None, Path::new("/a/roundup"), tmp, 101).unwrap();
    assert_ne!(first.socket, second.socket);
    assert!(first.socket.as_os_str().len() < 104);

    let too_deep = PathBuf::from(format!("/tmp/{}", "d".repeat(100)));
    let error = Config::locate(None, Path::new("/a/roundup"), &too_deep, 100).unwrap_err();
    assert!(error.contains("under 104"), "{error}");
}

#[test]
fn s1_the_daemon_is_the_env_override_else_the_rupd_beside_the_executable() {
    let tmp = Path::new("/tmp");
    let exe = Path::new("/apps/roundup.app/roundup");
    let beside = Config::locate(None, exe, tmp, 1).unwrap();
    assert_eq!(beside.rupd_bin, Path::new("/apps/roundup.app/rupd"));
    let overridden = Config::locate(Some(OsString::from("/fake/rupd")), exe, tmp, 1).unwrap();
    assert_eq!(overridden.rupd_bin, Path::new("/fake/rupd"));
}

#[test]
fn s1_a_path_that_is_not_a_directory_is_invalid_params_naming_it() {
    let fx = Fixture::new(SERVE_AND_WAIT);
    let (_app, webview) = app(fx.config.clone(), None);
    let file = fx.dir.path().join("rupd");

    let error = invoke(&webview, "open_project", path_arg(&file)).unwrap_err();

    assert_eq!(error["code"], code::INVALID_PARAMS);
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains(&file.display().to_string())
    );
}

#[test]
fn s1_a_daemon_that_exits_is_internal_and_carries_its_stderr() {
    let fx = Fixture::new("echo 'boom: cannot open the database' >&2\nexit 3");
    let (_app, webview) = app(fx.config.clone(), None);

    let error = invoke(&webview, "open_project", path_arg(&fx.project)).unwrap_err();

    assert_eq!(error["code"], code::INTERNAL);
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("boom: cannot open the database")
    );
}

#[test]
fn s1_a_daemon_silent_past_the_ready_bound_is_internal_and_carries_its_stderr() {
    let fx = Fixture::new("echo 'still starting' >&2\nexec cat");
    let mut config = fx.config.clone();
    config.ready_bound = Duration::from_millis(300);
    let (_app, webview) = app(config, None);

    let error = invoke(&webview, "open_project", path_arg(&fx.project)).unwrap_err();

    assert_eq!(error["code"], code::INTERNAL);
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("still starting")
    );
}

#[test]
fn s1_a_second_open_project_is_conflict() {
    let fx = Fixture::new(SERVE_AND_WAIT);
    fx.serve_real_daemon();
    let (_app, webview) = app(fx.config.clone(), None);
    open(&webview, &fx.project);

    let error = invoke(&webview, "open_project", path_arg(&fx.project)).unwrap_err();

    assert_eq!(error["code"], code::CONFLICT);
}

#[test]
fn s1_started_as_roundup_project_opens_that_project() {
    let fx = Fixture::new(SERVE_AND_WAIT);
    fx.serve_real_daemon();

    let (mut app, webview) = app(fx.config.clone(), Some(fx.project.clone()));
    // Tauri runs the setup hook, where the folder is opened, when the event loop starts.
    #[allow(deprecated)]
    app.run_iteration(|_, _| {});

    assert_eq!(
        invoke(&webview, "project", json!({})),
        Ok(json!({ "name": "my-project", "path": fx.project.display().to_string() }))
    );
}

#[test]
fn s1_the_webview_is_granted_dialogs_the_dock_badge_and_hearing_events_and_nothing_else() {
    let capabilities = Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
    let granted: BTreeSet<String> = std::fs::read_dir(capabilities)
        .unwrap()
        .flat_map(|entry| {
            let file: Value =
                serde_json::from_slice(&std::fs::read(entry.unwrap().path()).unwrap()).unwrap();
            file["permissions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        })
        .collect();

    let expected = [
        "dialog:allow-open",
        "dialog:allow-save",
        "core:window:allow-set-badge-count",
        "core:event:allow-listen",
        "core:event:allow-unlisten",
    ]
    .map(String::from)
    .into();
    assert_eq!(granted, expected);
}

fn tauri_config() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn s4_the_dev_window_loads_the_dev_server_on_the_port_vite_is_pinned_to() {
    let vite = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop/vite.config.ts");
    let vite = std::fs::read_to_string(vite).unwrap();

    assert_eq!(tauri_config()["build"]["devUrl"], "http://localhost:5173");
    assert!(
        vite.contains("port: 5173") && vite.contains("strictPort: true"),
        "vite must fail rather than move off the port devUrl names"
    );
}

#[test]
fn s4_the_built_app_embeds_the_directory_vite_builds_into() {
    assert_eq!(
        tauri_config()["build"]["frontendDist"],
        "../../apps/desktop/dist"
    );
}

#[test]
fn s2_rpc_ping_answers_pong() {
    let fx = Fixture::new(SERVE_AND_WAIT);
    fx.serve_real_daemon();
    let (_app, webview) = app(fx.config.clone(), None);
    open(&webview, &fx.project);

    let reply = invoke(
        &webview,
        "rpc",
        json!({ "method": "daemon.ping", "params": null }),
    );

    assert_eq!(reply, Ok(json!({ "pong": true })));
}

#[test]
fn s2_a_call_that_fails_in_the_daemon_fails_with_the_daemons_code_and_message() {
    let fx = Fixture::new(SERVE_AND_WAIT);
    fx.serve_real_daemon();
    let (_app, webview) = app(fx.config.clone(), None);
    open(&webview, &fx.project);
    let direct = tauri::async_runtime::block_on(async {
        let client = rpc::Client::connect(&fx.real_daemon_socket())
            .await
            .unwrap();
        client
            .request("todo.get", json!({ "id": 99 }))
            .await
            .unwrap_err()
    });

    let error = invoke(
        &webview,
        "rpc",
        json!({ "method": "todo.get", "params": { "id": 99 } }),
    )
    .unwrap_err();

    assert_eq!(
        error,
        json!({ "code": direct.code, "message": direct.message })
    );
    assert_eq!(error["code"], code::NOT_FOUND);
}

#[test]
fn s2_events_the_daemon_emits_in_one_burst_arrive_on_the_channel_in_order() {
    let fx = Fixture::new(SERVE_AND_WAIT);
    fx.serve_real_daemon();
    let (app, webview) = app(fx.config.clone(), None);
    open(&webview, &fx.project);
    let (channel, received) = event_channel();
    tauri::async_runtime::block_on(app.state::<AppState>().subscribe(channel)).unwrap();

    tauri::async_runtime::block_on(async {
        let client = rpc::Client::connect(&fx.real_daemon_socket())
            .await
            .unwrap();
        for n in 0..BURST {
            client
                .request("todo.create", json!({ "title": n.to_string() }))
                .await
                .unwrap();
        }
    });

    let titles: Vec<String> = (0..BURST)
        .map(|_| received.recv_timeout(WAIT).unwrap())
        .map(|event| {
            assert_eq!(event["name"], "todo.created");
            event["data"]["title"].as_str().unwrap().to_owned()
        })
        .collect();
    assert_eq!(
        titles,
        (0..BURST).map(|n| n.to_string()).collect::<Vec<_>>()
    );
}

#[test]
fn s2_subscribe_through_ipc_takes_a_channel_argument_and_answers_null() {
    let fx = Fixture::new(SERVE_AND_WAIT);
    fx.serve_real_daemon();
    let (_app, webview) = app(fx.config.clone(), None);
    open(&webview, &fx.project);
    let (channel, _received) = event_channel();

    let subscribed = invoke(&webview, "subscribe", json!({ "channel": channel }));
    let misnamed = invoke(&webview, "subscribe", json!({ "events": channel }));

    assert_eq!(subscribed, Ok(Value::Null));
    assert!(misnamed.is_err());
}

#[test]
fn s2_a_second_subscribe_replaces_the_first() {
    let fx = Fixture::new(SERVE_AND_WAIT);
    fx.serve_real_daemon();
    let (app, webview) = app(fx.config.clone(), None);
    open(&webview, &fx.project);
    let (first, first_received) = event_channel();
    let (second, second_received) = event_channel();
    let state = app.state::<AppState>();
    tauri::async_runtime::block_on(state.subscribe(first)).unwrap();
    tauri::async_runtime::block_on(state.subscribe(second)).unwrap();

    invoke(
        &webview,
        "rpc",
        json!({ "method": "todo.create", "params": { "title": "only" } }),
    )
    .unwrap();

    second_received.recv_timeout(WAIT).unwrap();
    assert!(first_received.try_recv().is_err());
}

#[test]
fn s2_before_a_project_is_open_rpc_and_subscribe_fail_at_once_with_conflict() {
    let fx = Fixture::new(SERVE_AND_WAIT);
    let (app, webview) = app(fx.config.clone(), None);

    let call = invoke(
        &webview,
        "rpc",
        json!({ "method": "daemon.ping", "params": null }),
    );
    let subscribe = tauri::async_runtime::block_on(app.state::<AppState>().subscribe(Channel::<
        contracts::Event,
    >::new(
        |_| Ok(())
    )));

    assert_eq!(call.unwrap_err()["code"], code::CONFLICT);
    let error = subscribe.unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("no Project is open"));
}

#[test]
fn s3_when_the_app_exits_the_daemons_stdin_closes() {
    let fx = Fixture::new("{link}\ncat > /dev/null\ntouch '{dir}/stdin-closed'");
    fx.serve_real_daemon();
    let (app, webview) = app(fx.config.clone(), None);
    open(&webview, &fx.project);
    let marker = fx.dir.path().join("stdin-closed");
    assert!(!marker.exists());

    handle_run_event(app.handle(), &RunEvent::Exit);

    eventually("the Daemon seeing its stdin close", || marker.exists());
}

#[test]
fn s3_when_the_daemon_exits_on_its_own_the_webview_gets_daemon_exited_with_its_code() {
    let fx = Fixture::new("{link}\nread go < '{dir}/go'\nexit 7");
    fx.serve_real_daemon();
    let (app, webview) = app(fx.config.clone(), None);
    let exited = daemon_exited(&app);
    open(&webview, &fx.project);

    fx.release();

    assert_eq!(exited.recv_timeout(WAIT).unwrap(), json!({ "code": 7 }));
}

#[test]
fn s3_a_daemon_ended_by_a_signal_has_a_null_code() {
    let fx = Fixture::new("{link}\nread go < '{dir}/go'\nkill -9 $$");
    fx.serve_real_daemon();
    let (app, webview) = app(fx.config.clone(), None);
    let exited = daemon_exited(&app);
    open(&webview, &fx.project);

    fx.release();

    assert_eq!(exited.recv_timeout(WAIT).unwrap(), json!({ "code": null }));
}

#[test]
fn s3_after_the_daemon_exits_rpc_fails_with_internal() {
    let fx = Fixture::new("{link}\nread go < '{dir}/go'\nexit 0");
    fx.serve_real_daemon();
    let (app, webview) = app(fx.config.clone(), None);
    let exited = daemon_exited(&app);
    open(&webview, &fx.project);
    fx.release();
    exited.recv_timeout(WAIT).unwrap();

    let error = invoke(
        &webview,
        "rpc",
        json!({ "method": "daemon.ping", "params": null }),
    )
    .unwrap_err();

    assert_eq!(error["code"], code::INTERNAL);
}

#[test]
fn s1_the_webview_may_listen_for_daemon_exited_through_the_real_capabilities() {
    let fx = Fixture::new(SERVE_AND_WAIT);
    let (_app, webview) = real_context_app(fx.config.clone());

    let listen = invoke(
        &webview,
        "plugin:event|listen",
        json!({ "event": "daemon-exited", "target": { "kind": "Any" }, "handler": 7 }),
    );

    let event_id = listen.expect("the capability grants core:event:allow-listen");
    let unlisten = invoke(
        &webview,
        "plugin:event|unlisten",
        json!({ "event": "daemon-exited", "eventId": event_id }),
    );
    assert_eq!(unlisten, Ok(Value::Null));
}

#[test]
fn s1_a_failed_roundup_project_start_exits_non_zero_without_panicking() {
    let fx = Fixture::new("echo 'cannot open the database' >&2\nexit 3");
    let exits = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&exits);
    let mut app = build(
        mock_builder(),
        fx.config.clone(),
        Some(fx.project.clone()),
        move |_, code| recorded.lock().unwrap().push(code),
    )
    .build(mock_context(noop_assets()))
    .unwrap();

    // Tauri runs the setup hook, where the project is opened, when the event loop starts.
    #[allow(deprecated)]
    app.run_iteration(|_, _| {});

    assert_eq!(*exits.lock().unwrap(), [1]);
}

#[test]
fn s1_a_socket_path_of_103_bytes_is_accepted_and_104_is_rejected() {
    let socket_name_len = "/roundup-1.sock".len();
    let dir_of = |socket_len: usize| {
        PathBuf::from(format!("/{}", "d".repeat(socket_len - socket_name_len - 1)))
    };

    let accepted = Config::locate(None, Path::new("/a/roundup"), &dir_of(103), 1).unwrap();
    let rejected = Config::locate(None, Path::new("/a/roundup"), &dir_of(104), 1);

    assert_eq!(accepted.socket.as_os_str().len(), 103);
    assert!(rejected.is_err());
}

#[test]
fn s1_a_daemon_that_accepts_connections_but_never_answers_ping_is_silent_past_the_bound() {
    let fx = Fixture::new(SERVE_AND_WAIT);
    let listener = std::os::unix::net::UnixListener::bind(fx.real_daemon_socket()).unwrap();
    std::thread::spawn(move || {
        let held: Vec<_> = listener.incoming().collect();
        drop(held);
    });
    let mut config = fx.config.clone();
    config.ready_bound = Duration::from_millis(300);
    let (_app, webview) = app(config, None);

    let error = invoke(&webview, "open_project", path_arg(&fx.project)).unwrap_err();

    assert_eq!(error["code"], code::INTERNAL);
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("did not answer daemon.ping")
    );
}

#[test]
fn s1_stderr_that_is_not_utf8_does_not_stop_the_tail() {
    let fx = Fixture::new("printf 'bad \\377 line\\n' >&2\necho 'after the bad line' >&2\nexit 3");
    let (_app, webview) = app(fx.config.clone(), None);

    let error = invoke(&webview, "open_project", path_arg(&fx.project)).unwrap_err();

    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("after the bad line")
    );
}

#[test]
fn s1_a_project_path_ending_in_dotdot_is_named_after_the_directory_it_reaches() {
    let fx = Fixture::new(SERVE_AND_WAIT);
    fx.serve_real_daemon();
    let (_app, webview) = app(fx.config.clone(), None);
    let through_dotdot = fx.project.join("..");

    let opened = invoke(&webview, "open_project", path_arg(&through_dotdot)).unwrap();

    let reached = fx.dir.path().file_name().unwrap().to_str().unwrap();
    assert_eq!(opened["name"], reached);
}

#[test]
fn s3_when_the_app_exits_the_socket_file_it_owned_is_gone() {
    let fx = Fixture::new(SERVE_AND_WAIT);
    fx.serve_real_daemon();
    let (app, webview) = app(fx.config.clone(), None);
    open(&webview, &fx.project);
    assert!(fx.config.socket.symlink_metadata().is_ok());

    handle_run_event(app.handle(), &RunEvent::Exit);

    assert!(fx.config.socket.symlink_metadata().is_err());
}
