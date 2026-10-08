//! A4: what an Agent's Claude Code is started with: a per-Agent settings file and a trusted cwd.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use agents::claude_code::{Launcher, Role};
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

/// `path` with `suffix` appended to its last component.
fn beside(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    name.into()
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
        Launcher::new(
            "fake-claude",
            self.claude_json.clone(),
            self.rup.clone(),
            None,
        )
    }

    fn prepare(&self, id: &str) -> Result<Vec<String>, rpc::RpcError> {
        self.prepare_as(id, Role::Agent)
    }

    fn prepare_as(&self, id: &str, role: Role) -> Result<Vec<String>, rpc::RpcError> {
        self.launcher()
            .prepare(&self.dir, id, "1", &self.cwd, None, role)
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
    assert_eq!(argv[3], "--mcp-config");
    assert_eq!(argv.len(), 7);
    for file in [&argv[2], &argv[4]] {
        assert!(Path::new(file).starts_with(s.dir.canonicalize().unwrap()));
    }
    let other = s.prepare("8").unwrap();
    assert_ne!((&argv[2], &argv[4]), (&other[2], &other[4]));
}

#[test]
fn a11_the_mcp_config_declares_one_stdio_server_that_runs_rup_mcp_for_the_agent() {
    let s = setup();
    let argv = s.prepare("7").unwrap();
    let config = read(Path::new(&argv[4]));
    assert_eq!(
        config,
        json!({"mcpServers": {"roundup": {
            "type": "stdio",
            "command": s.rup.to_string_lossy(),
            "args": ["mcp", "7"],
        }}})
    );
    assert_eq!(Path::new(&argv[4]).file_name().unwrap(), "7.mcp.json");
}

#[test]
fn a11_the_users_own_mcp_servers_stay_because_the_config_is_not_strict() {
    let s = setup();
    assert!(
        !s.prepare("7")
            .unwrap()
            .iter()
            .any(|arg| arg == "--strict-mcp-config")
    );
}

#[test]
fn a11_the_settings_allow_every_roundup_tool_so_none_raises_a_dialog() {
    let s = setup();
    let settings = read(Path::new(&s.prepare("7").unwrap()[2]));
    assert_eq!(
        settings["permissions"],
        json!({"allow": ["mcp__roundup__*"]})
    );
}

#[test]
fn a11_the_daemons_socket_is_in_neither_file() {
    let s = setup();
    let argv = s.prepare("7").unwrap();
    for file in [&argv[2], &argv[4]] {
        assert!(
            !std::fs::read_to_string(file)
                .unwrap()
                .contains("RUPD_SOCKET")
        );
    }
}

#[test]
fn a11_discarding_an_agent_deletes_both_of_its_files() {
    let s = setup();
    let argv = s.prepare("7").unwrap();
    Launcher::discard(&s.dir, "7").unwrap();
    assert!(
        ![&argv[2], &argv[4], &argv[6]]
            .iter()
            .any(|file| Path::new(file).exists())
    );
}

#[test]
fn a4_the_settings_hold_exactly_one_signal_command_per_state_event() {
    let s = setup();
    let mut settings = read(Path::new(&s.prepare("7").unwrap()[2]));
    settings.as_object_mut().unwrap().remove("permissions");
    assert!(s.rup.is_absolute() && s.rup.exists());
    let command = format!("ROUNDUP_ATTEMPT='1' '{}' signal '7'", s.rup.display());
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
fn a4_a_config_folder_that_does_not_exist_yet_is_created() {
    let s = setup();
    let config = s.root.path().join("config dir").join(".claude.json");
    let launcher = Launcher::new("fake-claude", config.clone(), s.rup.clone(), None);
    launcher
        .prepare(&s.dir, "1", "1", &s.cwd, None, Role::Agent)
        .unwrap();
    assert_eq!(
        read(&config)["projects"][&s.real(&s.cwd)]["hasTrustDialogAccepted"],
        true
    );
}

#[test]
fn a4_a_symlinked_cwd_is_trusted_under_its_resolved_path() {
    let s = setup();
    let link = s.cwd.parent().unwrap().join("link");
    std::os::unix::fs::symlink(&s.cwd, &link).unwrap();
    s.launcher()
        .prepare(&s.dir, "1", "1", &link, None, Role::Agent)
        .unwrap();
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
fn a13_a_symlinked_agents_directory_is_refused() {
    let s = setup();
    let elsewhere = s.root.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    let agents = s.dir.join("agents");
    std::os::unix::fs::symlink(&elsewhere, &agents).unwrap();

    let err = s.prepare("1").unwrap_err();

    assert_eq!(err.code, code::INVALID_PARAMS);
    let real_agents = s.dir.canonicalize().unwrap().join("agents");
    assert!(
        err.message
            .contains(&real_agents.to_string_lossy().into_owned()),
        "{}",
        err.message
    );
    assert!(agents.symlink_metadata().unwrap().file_type().is_symlink());
    assert!(std::fs::read_dir(&elsewhere).unwrap().next().is_none());
    assert!(!s.dir.join("1.settings.json").exists());
    assert!(!s.dir.join("1.mcp.json").exists());
    assert!(!s.claude_json.exists());
}

#[test]
fn a13_a_real_agents_directory_works_as_before() {
    let s = setup();
    let agents = s.dir.join("agents");
    std::fs::create_dir(&agents).unwrap();

    let argv = s.prepare("1").unwrap();

    let real_agents = s.real(&agents);
    assert!(Path::new(&argv[2]).starts_with(&real_agents));
    assert!(Path::new(&argv[4]).starts_with(&real_agents));
}

#[test]
fn a13_a_missing_agents_directory_is_created_as_before() {
    let s = setup();
    let agents = s.dir.join("agents");
    assert!(!agents.exists());

    s.prepare("1").unwrap();

    assert!(agents.is_dir());
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
                        .prepare(
                            &dir,
                            &(spawner * 10 + n).to_string(),
                            "1",
                            cwd,
                            None,
                            Role::Agent,
                        )
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
    let err = s
        .launcher()
        .prepare(&s.dir, "1", "1", &elsewhere, None, Role::Agent)
        .unwrap_err();
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
fn a4_a_cwd_that_is_a_file_is_the_callers_error() {
    let s = setup();
    let file = s.cwd.join("notes.txt");
    std::fs::write(&file, "").unwrap();
    let err = s
        .launcher()
        .prepare(&s.dir, "1", "1", &file, None, Role::Agent)
        .unwrap_err();
    assert_eq!(err.code, code::INVALID_PARAMS);
    assert!(!s.claude_json.exists());
}

#[test]
fn a4_a_relative_project_dir_still_gives_claude_an_absolute_settings_path() {
    let here = std::env::current_dir().unwrap();
    let root = tempfile::tempdir_in(&here).unwrap();
    let dir = root.path().join("proj").join(".roundup");
    let cwd = root.path().join("proj");
    std::fs::create_dir_all(&dir).unwrap();
    let rup = root.path().join("rup");
    std::fs::write(&rup, "").unwrap();
    let launcher = Launcher::new("fake-claude", root.path().join("claude.json"), rup, None);

    let argv = launcher
        .prepare(
            dir.strip_prefix(&here).unwrap(),
            "1",
            "1",
            &cwd,
            None,
            Role::Agent,
        )
        .unwrap();

    assert!(Path::new(&argv[2]).is_absolute(), "{}", argv[2]);
}

#[test]
fn a4_an_agent_id_that_is_not_a_node_id_is_refused_before_anything_is_written() {
    let s = setup();
    for id in ["1;touch pwned", "../1", "01", "+1", ""] {
        let err = s.prepare(id).unwrap_err();
        assert_eq!(err.code, code::INVALID_PARAMS, "{id:?}");
    }
    assert!(!s.dir.join("agents").exists());
    assert!(!s.claude_json.exists());
}

#[test]
fn a4_discarding_an_id_that_is_not_a_node_id_deletes_nothing() {
    let s = setup();
    s.prepare("1").unwrap();
    let outside = s.dir.join("x.settings.json");
    std::fs::write(&outside, "").unwrap();
    Launcher::discard(&s.dir, "../x").unwrap();
    assert!(outside.exists());
}

#[test]
fn a4_a_symlinked_config_whose_target_is_missing_stays_a_link() {
    let s = setup();
    std::os::unix::fs::symlink("dotfiles/claude.json", &s.claude_json).unwrap();
    s.prepare("1").unwrap();
    assert!(
        s.claude_json
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    let target = s.root.path().join("dotfiles").join("claude.json");
    assert_eq!(
        read(&target)["projects"][&s.real(&s.cwd)]["hasTrustDialogAccepted"],
        true
    );
}

#[test]
fn a4_a_leftover_at_the_old_fixed_temp_path_does_not_stop_trust() {
    let s = setup();
    std::fs::create_dir(beside(&s.claude_json, ".roundup-tmp")).unwrap();
    s.prepare("1").unwrap();
    assert_eq!(
        read(&s.claude_json)["projects"][&s.real(&s.cwd)]["hasTrustDialogAccepted"],
        true
    );
}

/// Claude Code 2.1.286 saves its config holding the directory `<config>.lock` and re-reads the
/// config under it; a trust written meanwhile would be lost to that save.
#[test]
fn a4_trust_waits_while_claude_holds_its_config_lock() {
    let s = setup();
    std::fs::write(&s.claude_json, "{}").unwrap();
    let lock = beside(&s.claude_json, ".lock");
    std::fs::create_dir(&lock).unwrap();
    let mut saving = read(&s.claude_json);
    let trusting = {
        let (launcher, dir, cwd) = (s.launcher(), s.dir.clone(), s.cwd.clone());
        std::thread::spawn(move || launcher.prepare(&dir, "1", "1", &cwd, None, Role::Agent))
    };

    // An observation window: while the lock is held, trust must not write.
    std::thread::sleep(Duration::from_millis(300));
    saving["theme"] = "dark".into();
    std::fs::write(&s.claude_json, saving.to_string()).unwrap();
    std::fs::remove_dir(&lock).unwrap();

    trusting.join().unwrap().unwrap();
    let config = read(&s.claude_json);
    assert_eq!(config["theme"], "dark");
    assert_eq!(
        config["projects"][&s.real(&s.cwd)]["hasTrustDialogAccepted"],
        true
    );
}

#[test]
fn a4_a_config_lock_its_holder_left_behind_is_broken_once_stale() {
    let s = setup();
    let lock = beside(&s.claude_json, ".lock");
    std::fs::create_dir(&lock).unwrap();
    let minute_ago = SystemTime::now() - Duration::from_secs(60);
    std::fs::File::open(&lock)
        .unwrap()
        .set_modified(minute_ago)
        .unwrap();

    s.prepare("1").unwrap();

    assert!(!lock.exists());
    assert_eq!(
        read(&s.claude_json)["projects"][&s.real(&s.cwd)]["hasTrustDialogAccepted"],
        true
    );
}

#[test]
fn a4_a_cwd_that_does_not_exist_is_the_callers_error() {
    let s = setup();
    let err = s
        .launcher()
        .prepare(&s.dir, "1", "1", &s.cwd.join("missing"), None, Role::Agent)
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
        Launcher::new(
            "/x/fake",
            "/h/.claude.json".into(),
            "/e/rup".into(),
            Some("/h".into())
        )
    );

    let launcher =
        Launcher::from_vars(vars(&[("CLAUDE_CONFIG_DIR", "/c"), ("HOME", "/h")]), exe).unwrap();
    assert_eq!(
        launcher,
        Launcher::new(
            "claude",
            "/c/.claude.json".into(),
            "/e/rup".into(),
            Some("/h".into())
        )
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
        Launcher::new(
            "claude",
            "/h/.claude.json".into(),
            "/e/rup".into(),
            Some("/h".into())
        )
    );
    assert_eq!(
        named,
        Launcher::new(
            "claude",
            "/h/.claude.json".into(),
            "/r/rup".into(),
            Some("/h".into())
        )
    );
}

#[test]
fn a4_an_empty_variable_counts_as_unset() {
    let exe = Path::new("/e/rupd");
    let launcher = Launcher::from_vars(
        vars(&[
            ("CLAUDE_CONFIG_DIR", ""),
            ("HOME", "/h"),
            ("ROUNDUP_RUP_BIN", ""),
        ]),
        exe,
    )
    .unwrap();
    assert_eq!(
        launcher,
        Launcher::new(
            "claude",
            "/h/.claude.json".into(),
            "/e/rup".into(),
            Some("/h".into())
        )
    );
    assert!(Launcher::from_vars(vars(&[("HOME", "")]), exe).is_err());
}

#[test]
fn a4_a_cwd_that_is_not_in_nfc_is_trusted_under_its_nfc_name() {
    let s = setup();
    let decomposed = s.cwd.parent().unwrap().join("cafe\u{301}");
    std::fs::create_dir(&decomposed).unwrap();

    s.launcher()
        .prepare(&s.dir, "1", "1", &decomposed, None, Role::Agent)
        .unwrap();

    let config = read(&s.claude_json);
    let keys: Vec<_> = config["projects"].as_object().unwrap().keys().collect();
    assert_eq!(keys.len(), 1);
    assert!(keys[0].ends_with("caf\u{e9}"), "{}", keys[0]);
}

#[test]
fn a4_the_home_folder_is_refused_and_never_trusted() {
    let s = setup();
    let project = s.dir.parent().unwrap();
    let launcher = Launcher::new(
        "fake-claude",
        s.claude_json.clone(),
        s.rup.clone(),
        Some(project.to_owned()),
    );

    let err = launcher
        .prepare(&s.dir, "1", "1", project, None, Role::Agent)
        .unwrap_err();

    assert_eq!(err.code, code::INVALID_PARAMS);
    assert!(!s.claude_json.exists());
    assert!(!s.dir.join("agents").exists());
    launcher
        .prepare(&s.dir, "1", "1", &s.cwd, None, Role::Agent)
        .unwrap();
}

#[test]
fn a4_a_symlink_planted_where_a_file_goes_is_replaced_not_written_through() {
    let s = setup();
    let agents = s.dir.join("agents");
    std::fs::create_dir(&agents).unwrap();
    for name in ["1.settings.json", "1.mcp.json"] {
        let victim = s.root.path().join(format!("victim-{name}"));
        std::fs::write(&victim, "export KEEP=1\n").unwrap();
        std::os::unix::fs::symlink(&victim, agents.join(name)).unwrap();
    }

    s.prepare("1").unwrap();

    for name in ["1.settings.json", "1.mcp.json"] {
        let victim = s.root.path().join(format!("victim-{name}"));
        let planted = agents.join(name);
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "export KEEP=1\n");
        assert!(!planted.symlink_metadata().unwrap().file_type().is_symlink());
    }
    assert!(read(&agents.join("1.settings.json"))["hooks"]["Stop"].is_array());
    assert!(read(&agents.join("1.mcp.json"))["mcpServers"]["roundup"].is_object());
}

fn brief(s: &Setup, id: &str, role: Role) -> String {
    let argv = s.prepare_as(id, role).unwrap();
    assert_eq!(argv[5], "--append-system-prompt-file");
    assert_eq!(
        Path::new(&argv[6]).file_name().unwrap(),
        format!("{id}.brief.md").as_str()
    );
    std::fs::read_to_string(&argv[6]).unwrap()
}

#[test]
fn e1_the_brief_names_the_agent_by_id_and_the_tools_and_holds_nothing_mutable() {
    let s = setup();
    let text = brief(&s, "7", Role::Agent);
    assert!(text.contains("Your Agent id is 7."));
    assert!(text.contains("supervises you"));
    for tool in ["todo_create", "todo_update", "pad_write", "pad_append"] {
        assert!(text.contains(&format!("`{tool}`")), "{tool}");
    }
    for unshipped in ["agent_context", "message_send", "ask_user", "agent_spawn"] {
        assert!(!text.contains(unshipped), "{unshipped}");
    }
    assert!(!text.contains(&s.cwd.to_string_lossy().into_owned()));
}

#[test]
fn e1_the_flag_adds_to_the_argv_and_leaves_the_users_files_alone() {
    let s = setup();
    std::fs::write(s.cwd.join("CLAUDE.md"), "mine").unwrap();
    let argv = s.prepare("7").unwrap();
    assert_eq!(argv[..5], s.prepare("7").unwrap()[..5]);
    assert_eq!(
        std::fs::read_to_string(s.cwd.join("CLAUDE.md")).unwrap(),
        "mine"
    );
    assert!(!s.cwd.join(".claude").exists());
}

#[test]
fn b24_only_a_door_brief_carries_the_role_guidance() {
    let s = setup();
    let door = brief(&s, "7", Role::Door);
    let agent = brief(&s, "8", Role::Agent);
    assert!(door.contains("coordinating Door"));
    assert!(door.contains("ask in this Terminal"));
    assert!(!agent.contains("Door"));
    assert!(!agent.contains("Your role"));
}

#[test]
fn e1_preparing_the_same_id_and_role_twice_writes_the_same_text() {
    let s = setup();
    assert_eq!(brief(&s, "7", Role::Door), brief(&s, "7", Role::Door));
}
