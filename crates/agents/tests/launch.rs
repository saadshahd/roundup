//! A4: what an Agent's Claude Code is started with: a per-Agent settings file and a trusted cwd.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agents::claude_code::Launcher;
use rpc::code;
use serde_json::{Value, json};

const EVENTS: [&str; 9] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "PostToolUseFailure",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

fn read(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A Project folder `<root>/proj` with its `.roundup` directory, and a `rup` to point hooks at.
struct Setup {
    root: tempfile::TempDir,
    dir: PathBuf,
    cwd: PathBuf,
    rup: PathBuf,
    claude_json: PathBuf,
}

fn setup() -> Setup {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("proj");
    let dir = project.join(".roundup");
    let cwd = project.join("work");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir(&cwd).unwrap();
    let rup = root.path().join("bin dir").join("rup");
    std::fs::create_dir_all(rup.parent().unwrap()).unwrap();
    std::fs::write(&rup, "").unwrap();
    let claude_json = root.path().join("claude.json");
    Setup {
        root,
        dir,
        cwd,
        rup,
        claude_json,
    }
}

impl Setup {
    fn launcher(&self) -> Launcher {
        Launcher::new("fake-claude", self.claude_json.clone(), self.rup.clone())
    }

    fn prepare(&self, id: &str) -> Result<Vec<String>, rpc::RpcError> {
        self.launcher().prepare(&self.dir, id, &self.cwd)
    }

    fn real(&self, path: &Path) -> String {
        path.canonicalize().unwrap().to_string_lossy().into_owned()
    }
}

#[test]
fn a4_the_argv_runs_claude_with_a_per_agent_settings_file() {
    let s = setup();
    let argv = s.prepare("7").unwrap();
    assert_eq!(argv[..2], ["fake-claude", "--settings"]);
    assert_eq!(argv.len(), 3);
    assert!(Path::new(&argv[2]).starts_with(&s.dir));
    assert_ne!(argv[2], s.prepare("8").unwrap()[2]);
}

#[test]
fn a4_the_settings_hold_exactly_one_signal_command_per_state_event() {
    let s = setup();
    let settings = read(Path::new(&s.prepare("7").unwrap()[2]));
    assert!(s.rup.is_absolute() && s.rup.exists());
    let command = format!("'{}' signal 7", s.rup.display());
    let entry = json!([{"hooks": [{"type": "command", "command": command, "timeout": 5}]}]);
    let expected: serde_json::Map<String, Value> = EVENTS
        .iter()
        .map(|event| ((*event).to_owned(), entry.clone()))
        .collect();
    // The late Notification and the spurious SubagentStop are not among the events.
    assert_eq!(settings, json!({ "hooks": expected }));
}

#[test]
fn a4_a_rup_that_is_not_there_fails_loudly() {
    let s = setup();
    std::fs::remove_file(&s.rup).unwrap();
    let err = s.prepare("7").unwrap_err();
    assert_eq!(err.code, code::INTERNAL);
    assert!(err.message.contains("rup"), "{}", err.message);
}

#[test]
fn a4_the_cwd_is_pre_trusted_and_nothing_else_in_the_config_changes() {
    let s = setup();
    let real = s.real(&s.cwd);
    std::fs::write(
        &s.claude_json,
        json!({"theme": "dark", "projects": {"/other": {"allowedTools": ["x"]}, real.clone(): {"allowedTools": ["y"]}}}).to_string(),
    )
    .unwrap();
    s.prepare("1").unwrap();
    let config = read(&s.claude_json);
    assert_eq!(config["theme"], "dark");
    assert_eq!(config["projects"]["/other"], json!({"allowedTools": ["x"]}));
    assert_eq!(
        config["projects"][&real],
        json!({"allowedTools": ["y"], "hasTrustDialogAccepted": true})
    );
}

#[test]
fn a4_a_missing_config_is_created_with_the_trust() {
    let s = setup();
    s.prepare("1").unwrap();
    let real = s.real(&s.cwd);
    assert_eq!(
        read(&s.claude_json)["projects"][&real]["hasTrustDialogAccepted"],
        true
    );
}

#[test]
fn a4_a_symlinked_cwd_is_trusted_under_its_resolved_path() {
    let s = setup();
    let link = s.cwd.parent().unwrap().join("link");
    std::os::unix::fs::symlink(&s.cwd, &link).unwrap();
    s.launcher().prepare(&s.dir, "1", &link).unwrap();
    let config = read(&s.claude_json);
    let keys: Vec<_> = config["projects"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys, [s.real(&s.cwd)]);
}

#[test]
fn a4_a_symlinked_config_is_written_through_and_stays_a_link() {
    let s = setup();
    let target = s.root.path().join("dotfiles-claude.json");
    std::fs::write(&target, r#"{"theme": "dark"}"#).unwrap();
    std::os::unix::fs::symlink(&target, &s.claude_json).unwrap();
    s.prepare("1").unwrap();
    assert!(
        s.claude_json
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(read(&target)["theme"], "dark");
    assert_eq!(
        read(&target)["projects"][&s.real(&s.cwd)]["hasTrustDialogAccepted"],
        true
    );
}

#[test]
fn a4_concurrent_spawns_each_get_their_cwd_trusted() {
    let s = setup();
    let launcher = Arc::new(s.launcher());
    let gate = Arc::new(std::sync::Barrier::new(8));
    let cwds: Vec<Vec<PathBuf>> = (0..8)
        .map(|spawner| {
            (0..5)
                .map(|n| s.dir.parent().unwrap().join(format!("w{spawner}-{n}")))
                .collect()
        })
        .collect();
    cwds.iter()
        .flatten()
        .for_each(|cwd| std::fs::create_dir(cwd).unwrap());
    let threads: Vec<_> = cwds
        .iter()
        .cloned()
        .enumerate()
        .map(|(spawner, mine)| {
            let (launcher, gate, dir) = (Arc::clone(&launcher), Arc::clone(&gate), s.dir.clone());
            std::thread::spawn(move || {
                gate.wait();
                for (n, cwd) in mine.iter().enumerate() {
                    launcher
                        .prepare(&dir, &format!("{spawner}-{n}"), cwd)
                        .unwrap();
                }
            })
        })
        .collect();
    threads.into_iter().for_each(|t| t.join().unwrap());
    let config = read(&s.claude_json);
    for cwd in cwds.iter().flatten() {
        assert_eq!(
            config["projects"][&s.real(cwd)]["hasTrustDialogAccepted"],
            true
        );
    }
}

#[test]
fn a4_only_a_cwd_inside_the_project_folder_is_trusted() {
    let s = setup();
    let elsewhere = s.root.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    let err = s.launcher().prepare(&s.dir, "1", &elsewhere).unwrap_err();
    assert_eq!(err.code, code::INVALID_PARAMS);
    assert!(!s.claude_json.exists());
    assert!(!s.dir.join("agents").exists());
}

#[test]
fn a4_an_unreadable_config_fails_loudly_and_is_left_as_it_was() {
    let s = setup();
    std::fs::write(&s.claude_json, "{ not json").unwrap();
    let err = s.prepare("1").unwrap_err();
    assert_eq!(err.code, code::INTERNAL);
    assert_eq!(
        std::fs::read_to_string(&s.claude_json).unwrap(),
        "{ not json"
    );
}

#[test]
fn a4_a_cwd_that_does_not_exist_is_the_callers_error() {
    let s = setup();
    let err = s
        .launcher()
        .prepare(&s.dir, "1", &s.cwd.join("missing"))
        .unwrap_err();
    assert_eq!(err.code, code::INVALID_PARAMS);
}

fn vars<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| OsString::from(value))
    }
}

#[test]
fn a4_the_binary_and_config_location_come_from_the_environment() {
    let exe = Path::new("/e/rupd");
    let launcher = Launcher::from_vars(
        vars(&[("ROUNDUP_CLAUDE_BIN", "/x/fake"), ("HOME", "/h")]),
        exe,
    )
    .unwrap();
    assert_eq!(
        launcher,
        Launcher::new("/x/fake", "/h/.claude.json".into(), "/e/rup".into())
    );

    let launcher =
        Launcher::from_vars(vars(&[("CLAUDE_CONFIG_DIR", "/c"), ("HOME", "/h")]), exe).unwrap();
    assert_eq!(
        launcher,
        Launcher::new("claude", "/c/.claude.json".into(), "/e/rup".into())
    );

    assert!(Launcher::from_vars(vars(&[]), exe).is_err());
}

#[test]
fn a4_rup_is_the_one_beside_the_running_daemon_unless_the_environment_says_otherwise() {
    let exe = Path::new("/e/rupd");
    let beside = Launcher::from_vars(vars(&[("HOME", "/h")]), exe).unwrap();
    let named =
        Launcher::from_vars(vars(&[("HOME", "/h"), ("ROUNDUP_RUP_BIN", "/r/rup")]), exe).unwrap();
    assert_eq!(
        beside,
        Launcher::new("claude", "/h/.claude.json".into(), "/e/rup".into())
    );
    assert_eq!(
        named,
        Launcher::new("claude", "/h/.claude.json".into(), "/r/rup".into())
    );
}
