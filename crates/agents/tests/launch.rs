//! A4: what an Agent's Claude Code is started with: a per-Agent settings file and a trusted cwd.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use agents::claude_code::Launcher;
use rpc::code;
use serde_json::{Value, json};

fn read(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

struct Setup {
    dir: tempfile::TempDir,
    cwd: PathBuf,
    claude_json: PathBuf,
}

fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("work");
    std::fs::create_dir(&cwd).unwrap();
    let claude_json = dir.path().join("claude.json");
    Setup {
        dir,
        cwd,
        claude_json,
    }
}

impl Setup {
    fn launcher(&self) -> Launcher {
        Launcher::new("fake-claude", self.claude_json.clone())
    }

    fn prepare(&self, id: &str) -> Result<Vec<String>, rpc::RpcError> {
        self.launcher().prepare(self.dir.path(), id, &self.cwd)
    }
}

#[test]
fn a4_the_argv_runs_claude_with_a_per_agent_settings_file() {
    let s = setup();
    let argv = s.prepare("7").unwrap();
    assert_eq!(argv[..2], ["fake-claude", "--settings"]);
    assert_eq!(argv.len(), 3);
    assert!(Path::new(&argv[2]).starts_with(s.dir.path()));
    assert_ne!(argv[2], s.prepare("8").unwrap()[2]);
}

#[test]
fn a4_every_state_event_runs_rup_hook_for_that_agent() {
    let s = setup();
    let settings = read(Path::new(&s.prepare("7").unwrap()[2]));
    let hooks = settings["hooks"].as_object().unwrap();
    for event in [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PermissionRequest",
        "PostToolUse",
        "PostToolUseFailure",
        "Stop",
        "StopFailure",
        "SessionEnd",
    ] {
        let entry = &hooks[event][0]["hooks"][0];
        assert_eq!(entry["type"], "command", "{event}");
        assert_eq!(entry["command"], "rup hook 7", "{event}");
    }
    // The late Notification and the spurious SubagentStop are never subscribed to.
    assert!(!hooks.contains_key("Notification") && !hooks.contains_key("SubagentStop"));
}

#[test]
fn a4_the_cwd_is_pre_trusted_and_nothing_else_in_the_config_changes() {
    let s = setup();
    let real = s.cwd.canonicalize().unwrap().to_string_lossy().into_owned();
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
    let real = s.cwd.canonicalize().unwrap().to_string_lossy().into_owned();
    assert_eq!(
        read(&s.claude_json)["projects"][&real]["hasTrustDialogAccepted"],
        true
    );
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
        .prepare(s.dir.path(), "1", &s.dir.path().join("missing"))
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
    let launcher =
        Launcher::from_vars(vars(&[("ROUNDUP_CLAUDE_BIN", "/x/fake"), ("HOME", "/h")])).unwrap();
    assert_eq!(launcher, Launcher::new("/x/fake", "/h/.claude.json".into()));

    let launcher =
        Launcher::from_vars(vars(&[("CLAUDE_CONFIG_DIR", "/c"), ("HOME", "/h")])).unwrap();
    assert_eq!(launcher, Launcher::new("claude", "/c/.claude.json".into()));

    assert!(Launcher::from_vars(vars(&[])).is_err());
}
