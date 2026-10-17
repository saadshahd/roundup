//! What an Agent's Claude Code is started with: a per-Agent settings file whose command hook is
//! `rup signal <agent-id>`, a per-Agent MCP config that runs `rup mcp <agent-id>`, and a pre-trusted
//! working directory (no hook fires for the trust dialog).

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use contracts::agent::Extras;
use rpc::{OpenError, RpcError, code};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tokio::sync::OnceCell;
use unicode_normalization::UnicodeNormalization;

use super::brief::{self, Role};
use super::extras;
use super::plugins;

/// F2, M11: the only tools a Door is started with; `ToolSearch` is there because its Brief tells it to load the deferred roundup tools with it. Every other tool Claude Code has, `Bash`, `Edit`,
/// `Write` and `Agent` among them, is absent; the MCP servers' tools stay (F1).
pub const DOOR_TOOLS: &str = "Read,Grep,Glob,ToolSearch";

/// M2219: a Door's tools with no roundup server; its Brief no longer tells it to load tools, so `ToolSearch` goes.
pub const DOOR_TOOLS_NO_MCP: &str = "Read,Grep,Glob";

/// LC1: a scout's tools; it reads and searches, and `Bash` is how it searches a repository.
pub const SCOUT_TOOLS: &str = "Read,Grep,Glob,Bash,ToolSearch";

/// M2219 for a scout.
pub const SCOUT_TOOLS_NO_MCP: &str = "Read,Grep,Glob,Bash";

/// LC1: what a Role's child starts with, beyond LC2's settings, which every row shares.
pub struct Profile {
    /// `--tools` with roundup's MCP server and without it; `None` is every tool.
    pub tools: Option<(&'static str, &'static str)>,
    /// Skills are on: no `--disable-slash-commands`, and the Project's skills are listed.
    pub skills: bool,
    /// `--plugin-dir` for `hope`, `hunch` and `hold` (LC5).
    pub bundled_plugins: bool,
    /// The user's global `CLAUDE.md` is appended to the Brief file (LC5).
    pub global_instructions: bool,
}

/// LC1: the one table of launch profiles, a row for each lean Role.
pub const PROFILES: [(Role, Profile); 3] = [
    (
        Role::Door,
        Profile {
            tools: Some((DOOR_TOOLS, DOOR_TOOLS_NO_MCP)),
            // A41: a `/command` sent to a Door runs there.
            skills: true,
            bundled_plugins: false,
            global_instructions: false,
        },
    ),
    (
        Role::Scout,
        Profile {
            tools: Some((SCOUT_TOOLS, SCOUT_TOOLS_NO_MCP)),
            skills: false,
            bundled_plugins: false,
            global_instructions: false,
        },
    ),
    (
        Role::Agent,
        Profile {
            tools: None,
            skills: true,
            bundled_plugins: true,
            global_instructions: true,
        },
    ),
];

/// LC1: `role`'s row.
pub fn profile(role: Role) -> &'static Profile {
    &PROFILES
        .iter()
        .find(|(row, _)| *row == role)
        .expect("PROFILES has a row for every Role")
        .1
}

/// LC2: the settings keys that sign a child in.
const LOGIN_KEYS: [&str; 5] = [
    "apiKeyHelper",
    "awsAuthRefresh",
    "awsCredentialExport",
    "forceLoginMethod",
    "forceLoginOrgUUID",
];

/// LC2: the user's settings a lean child keeps.
struct Carried {
    allow: Vec<Value>,
    deny: Vec<Value>,
    login: Map<String, Value>,
}

/// F4: what the shim for a vendor program prints on stderr before it exits 1.
const SHIM_MESSAGE: &str = "roundup: start an agent with agent_spawn (a Door) or ask the user";

/// The hook events that carry state. Notification and SubagentStop are left out on purpose: the
/// first arrives about 6 s late, the second fires spuriously (ADR 0006). H15's replay rule would
/// also drop an event here when removing its Signal from every fixture in
/// `spikes/hooks-state/log*.jsonl` and from D2, replayed with the fixture's titles alongside what
/// is left, leaves every Status unchanged; `crates/agents/tests/state_events.rs` checks the rule
/// against this list itself and finds no event prunable, since A2's star hold reads `PreToolUse`
/// to tell a star that may precede a dialog from one that may not (`mod.rs`). H15's saving is a
/// current-thread tokio runtime for `rup signal` alone (`crates/rup/src/main.rs`).
/// It is `pub` only so `crates/agents/tests/state_events.rs` can read it.
pub const STATE_EVENTS: [&str; 9] = [
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

/// A config lock this old was left by a holder that died: Claude Code keeps `proper-lockfile`'s
/// 10 s default, and a live holder refreshes the lock's mtime every 5 s.
const STALE: Duration = Duration::from_secs(10);

/// One thread at a time takes Claude's config lock: two that both found it stale would each
/// break it, and the second break would remove the lock the first had just taken.
static TRUSTING: Mutex<()> = Mutex::new(());

#[derive(Clone)]
pub struct Launcher {
    pub(super) bin: String,
    /// Claude Code's own config file, where it remembers which folders are trusted.
    claude_json: PathBuf,
    pub(super) conversation_dir: PathBuf,
    /// The `rup` the hooks run, by absolute path: a hook's PATH is not ours to trust.
    rup: PathBuf,
    /// The user's home folder, never trusted: Claude never saves trust for it, and a saved one
    /// would trust every non-git folder below it.
    home: Option<PathBuf>,
    /// M11: the Daemon's environment already holds `ENABLE_TOOL_SEARCH`, which is the user's and left alone.
    user_tool_search: bool,
    plugins: Arc<OnceCell<Delivery>>,
    /// The installed Claude Code's version, once `claude --version` has answered (B51).
    pub(super) version: Arc<OnceCell<String>>,
    /// B50: the private folder each Agent's messaging socket is made in, made at the first
    /// launch; gone with the Launcher.
    sockets: Arc<std::sync::OnceLock<tempfile::TempDir>>,
}

const QUESTION_FILES: [(&str, &str); 3] = [
    (
        ".claude-plugin/plugin.json",
        r#"{"name":"roundup-question","version":"0.0.1"}"#,
    ),
    ("hooks/hooks.json", r#"{"modules":["./question.js"]}"#),
    ("hooks/question.js", include_str!("question.js")),
];

struct Delivery {
    data: PathBuf,
    plugins: plugins::Plugins,
    runtime: plugins::Runtime,
    question: tempfile::TempDir,
}

// Equality describes launch configuration, independent of the cached runtime probe.
impl PartialEq for Launcher {
    fn eq(&self, other: &Self) -> bool {
        (
            &self.bin,
            &self.claude_json,
            &self.conversation_dir,
            &self.rup,
            &self.home,
            self.user_tool_search,
        ) == (
            &other.bin,
            &other.claude_json,
            &other.conversation_dir,
            &other.rup,
            &other.home,
            other.user_tool_search,
        )
    }
}
impl Eq for Launcher {}
impl std::fmt::Debug for Launcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Launcher")
            .field("bin", &self.bin)
            .field("claude_json", &self.claude_json)
            .field("rup", &self.rup)
            .finish_non_exhaustive()
    }
}

/// A36: the Model and Effort an Agent is started on; `None` is passed to nothing, so the user's
/// own Claude Code default applies.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pair {
    pub model: Option<String>,
    pub effort: Option<String>,
    /// M2219: launch with no roundup MCP server; false (the default) keeps it.
    pub mcp_off: bool,
    /// LC2, LC4, LC7: a child a Door or an Agent started launches lean; `None` is an Agent the
    /// user started, which keeps the user's full setup (LC6). A Door is always lean.
    pub lean: Option<Lean>,
}

/// What distinguishes a lean child within its Role, kept in the Project's data folder beside its
/// settings so that every resume launches the same.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lean {
    /// LC4: a scout, not a worker.
    #[serde(default)]
    pub scout: bool,
    /// LC7, LC8: the extras its Door named.
    #[serde(default)]
    pub extras: Extras,
    /// LC8: the next launch applies a changed Brief, so it passes `--system-prompt-snapshot off`.
    #[serde(default)]
    pub fresh_brief: bool,
}

impl Pair {
    /// A36: the variables that pin a set value above whatever the Daemon's environment holds,
    /// since Claude Code reads them before the flag and before the settings `model`.
    pub fn env(&self) -> BTreeMap<String, String> {
        [
            (super::models::MODEL_VARIABLE, &self.model),
            (super::models::EFFORT_VARIABLE, &self.effort),
        ]
        .into_iter()
        .filter_map(|(name, value)| Some((name.to_owned(), value.clone()?)))
        .collect()
    }
}

/// The Project folder, which bounds the trusted working directory, and the Project's data folder
/// (N1), where the launch files go.
#[derive(Clone, Copy)]
pub struct Folders<'a> {
    pub project: &'a Path,
    pub data: &'a Path,
}

impl Launcher {
    /// A39 uses the same binary as every launch, through its own Placement seam.
    pub fn placement(&self) -> super::PlacementCall {
        super::PlacementCall {
            bin: self.bin.clone(),
        }
    }

    pub fn new(
        bin: impl Into<String>,
        claude_json: PathBuf,
        rup: PathBuf,
        home: Option<PathBuf>,
    ) -> Self {
        let conversation_dir = match &home {
            Some(home) if claude_json == home.join(".claude.json") => home.join(".claude"),
            _ => claude_json.parent().unwrap_or(Path::new(".")).to_path_buf(),
        };
        Self {
            conversation_dir,
            bin: bin.into(),
            claude_json,
            rup,
            home,
            user_tool_search: false,
            plugins: Arc::new(OnceCell::new()),
            version: Arc::new(OnceCell::new()),
            sockets: Arc::new(std::sync::OnceLock::new()),
        }
    }

    pub fn from_env() -> Result<Self, OpenError> {
        Self::from_vars(|name| std::env::var_os(name), &std::env::current_exe()?)
    }

    /// `ROUNDUP_CLAUDE_BIN` names the program (default `claude`); `CLAUDE_CONFIG_DIR`, else `HOME`,
    /// says where its config lives; `ROUNDUP_RUP_BIN`, else the `rup` beside `exe` (the running
    /// Daemon), is what the hooks run. A variable set to nothing counts as unset.
    pub fn from_vars(
        var: impl Fn(&str) -> Option<OsString>,
        exe: &Path,
    ) -> Result<Self, OpenError> {
        let var = |name: &str| var(name).filter(|value| !value.is_empty());
        let bin = var("ROUNDUP_CLAUDE_BIN")
            .map_or_else(|| "claude".into(), |bin| bin.to_string_lossy().into_owned());
        let home = var("HOME").map(PathBuf::from);
        let config_dir = var("CLAUDE_CONFIG_DIR").map(PathBuf::from);
        let claude_json = match (&config_dir, &home) {
            (Some(dir), _) => dir.join(".claude.json"),
            (None, Some(home)) => home.join(".claude.json"),
            (None, None) => return Err("neither CLAUDE_CONFIG_DIR nor HOME is set".into()),
        };
        let rup = var("ROUNDUP_RUP_BIN").map_or_else(|| exe.with_file_name("rup"), PathBuf::from);
        let mut launcher = Self::new(bin, claude_json, rup, home);
        if let Some(dir) = config_dir {
            launcher.conversation_dir = dir;
        }
        launcher.user_tool_search = var("ENABLE_TOOL_SEARCH").is_some();
        Ok(launcher)
    }

    /// Delete what `prepare` wrote for Agent `id`, for an Agent that never started.
    pub fn discard(dir: &Path, id: &str) -> std::io::Result<()> {
        // `prepare` writes nothing for an id that is not a node id.
        let Some(id) = parse_id(id) else {
            return Ok(());
        };
        for file in [
            settings_path(dir, id),
            mcp_config_path(dir, id),
            brief_path(dir, id),
            lean_path(dir, id),
        ] {
            match std::fs::remove_file(file) {
                Err(err) if err.kind() != ErrorKind::NotFound => return Err(err),
                _ => {}
            }
        }
        match std::fs::remove_dir_all(extras_dir(dir, id)) {
            Err(err) if err.kind() != ErrorKind::NotFound => return Err(err),
            _ => {}
        }
        remove_shims(dir, id)
    }

    /// LC4, LC7: remember that Agent `id` launches lean as `lean` says, for every launch after.
    pub fn save_lean(dir: &Path, id: &str, lean: &Lean) -> Result<(), RpcError> {
        let id = parse_id(id).ok_or_else(|| RpcError::internal("not a node id"))?;
        let value = serde_json::to_value(lean).map_err(RpcError::internal)?;
        replace_file(&lean_path(dir, id), &value).map_err(RpcError::internal)
    }

    /// What `save_lean` kept; `None` for an Agent that launches with the user's full setup.
    pub fn load_lean(dir: &Path, id: &str) -> Option<Lean> {
        let bytes = std::fs::read(lean_path(dir, parse_id(id)?)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// The names LC7 resolves against, for a Project at `project`.
    pub fn extras_sources<'a>(&'a self, project: &'a Path) -> extras::Sources<'a> {
        extras::Sources {
            claude_dir: &self.conversation_dir,
            claude_json: &self.claude_json,
            project,
        }
    }

    /// Write Agent `id`'s settings, MCP config and Brief (E1, for its `role`) under `dir` (the Project's data folder, N1), trust
    /// `cwd`, and return the argv that starts it. Only a `cwd` inside the Project folder is trusted: Claude's config
    /// is the user's, and this is the only grant roundup makes in it. A22: `resume` is the saved
    /// conversation id to continue, added to argv as `--resume <id>`; `None` starts fresh.
    pub fn prepare(
        &self,
        folders: Folders<'_>,
        id: &str,
        attempt: &str,
        cwd: &Path,
        resume: Option<&str>,
        role: Role,
    ) -> Result<Vec<String>, RpcError> {
        self.prepare_on(folders, id, attempt, cwd, resume, role, &Pair::default())
    }

    /// Like `prepare`, on `pair` (A36): `--model` and `--effort` for each value that is set.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_on(
        &self,
        folders: Folders<'_>,
        id: &str,
        attempt: &str,
        cwd: &Path,
        resume: Option<&str>,
        role: Role,
        pair: &Pair,
    ) -> Result<Vec<String>, RpcError> {
        let Folders { project, data: dir } = folders;
        let invalid = |message: String| RpcError::new(code::INVALID_PARAMS, message);
        let id = parse_id(id).ok_or_else(|| invalid(format!("{id:?} is not a node id")))?;
        let cwd = cwd
            .canonicalize()
            .map_err(|err| invalid(format!("cwd {} is unusable: {err}", cwd.display())))?;
        if !cwd.is_dir() {
            return Err(invalid(format!("cwd {} is not a folder", cwd.display())));
        }
        if self
            .home
            .as_ref()
            .is_some_and(|home| home.canonicalize().is_ok_and(|home| home == cwd))
        {
            return Err(invalid(format!(
                "cwd {} is the home folder, which is never trusted",
                cwd.display()
            )));
        }
        // Claude reads the settings path from its own cwd, so it must be absolute.
        let dir = dir
            .canonicalize()
            .map_err(|err| RpcError::internal(format!("{}: {err}", dir.display())))?;
        let project = project
            .canonicalize()
            .map_err(|err| RpcError::internal(format!("{}: {err}", project.display())))?;
        if !cwd.starts_with(&project) {
            return Err(invalid(format!(
                "cwd {} is outside the project folder {}",
                cwd.display(),
                project.display()
            )));
        }
        if !self.rup.is_absolute() || !self.rup.is_file() {
            return Err(RpcError::internal(format!(
                "rup not found at {}: hooks could not signal",
                self.rup.display()
            )));
        }
        // Refuse a symlinked agents directory (A13) before writing anything, through the
        // link or beside it: the Project's own files are not ours to trust, and a link
        // could land a write anywhere.
        let agents_dir = agents_dir(&dir);
        if is_symlink(&agents_dir) {
            return Err(invalid(format!("{} is a symlink", agents_dir.display())));
        }
        let settings = settings_path(&dir, id);
        let mcp_config = mcp_config_path(&dir, id);
        let shims = shim_dir(&dir, id);
        if is_symlink(&shims) {
            return Err(invalid(format!("{} is a symlink", shims.display())));
        }
        let path = self.shell_path((role == Role::Door).then_some(shims.as_path()));
        if role == Role::Door {
            write_shims(&shims).map_err(RpcError::internal)?;
        }
        let mcp = !pair.mcp_off;
        let lean = lean_of(role, pair);
        let role = match (&lean, role) {
            (Some(lean), Role::Agent) if lean.scout => Role::Scout,
            _ => role,
        };
        let delivery = match &lean {
            Some(lean) => extras::resolve(&self.extras_sources(&project), &lean.extras)?,
            None => extras::Delivery::default(),
        };
        let carried = lean.as_ref().map(|_| self.user_carried());
        replace_file(
            &settings,
            &self.settings_json(id, attempt, &path, mcp, carried.as_ref()),
        )
        .map_err(RpcError::internal)?;
        let mut servers = Map::new();
        if mcp {
            servers.extend(self.mcp_servers(id));
        }
        servers.extend(delivery.servers.clone());
        let has_config = !servers.is_empty();
        if has_config {
            replace_file(&mcp_config, &json!({ "mcpServers": servers }))
                .map_err(RpcError::internal)?;
        } else if is_symlink(&mcp_config) {
            return Err(invalid(format!("{} is a symlink", mcp_config.display())));
        } else {
            // An earlier launch with MCP on left its config; the Agent must not find it.
            match std::fs::remove_file(&mcp_config) {
                Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
                    return Err(RpcError::internal(err));
                }
                _ => {}
            }
        }
        let brief_file = brief_path(&dir, id);
        let mut brief = brief::text(role, mcp);
        if lean.is_some() && profile(role).global_instructions {
            self.append_global_instructions(&mut brief);
        }
        replace_text(&brief_file, &brief).map_err(RpcError::internal)?;
        self.trust(&cwd)?;
        let mut argv = vec![
            self.bin.clone(),
            "--settings".into(),
            settings.to_string_lossy().into_owned(),
        ];
        if has_config {
            argv.push("--mcp-config".into());
            argv.push(mcp_config.to_string_lossy().into_owned());
        }
        argv.push("--append-system-prompt-file".into());
        argv.push(brief_file.to_string_lossy().into_owned());
        if let Some(lean) = &lean {
            // LC2: nothing of the user's settings, plugins or servers but what `--settings` copies.
            argv.extend(["--strict-mcp-config", "--setting-sources", "project"].map(String::from));
            let row = profile(role);
            if let Some((with, without)) = row.tools {
                argv.push("--tools".into());
                argv.push(if mcp { with } else { without }.into());
            }
            let skills = !delivery.skills.is_empty();
            if !row.skills && !skills {
                argv.push("--disable-slash-commands".into());
            }
            if skills {
                let folder = self.extras_plugin(&dir, id, &delivery.skills)?;
                argv.extend(["--plugin-dir".into(), folder.to_string_lossy().into_owned()]);
            }
            for plugin in &delivery.plugins {
                argv.extend(["--plugin-dir".into(), plugin.to_string_lossy().into_owned()]);
            }
            if lean.fresh_brief && resume.is_some() {
                argv.extend(["--system-prompt-snapshot", "off"].map(String::from));
            }
        }
        for (flag, value) in [("--model", &pair.model), ("--effort", &pair.effort)] {
            if let Some(value) = value {
                argv.push(flag.into());
                argv.push(value.clone());
            }
        }
        // B50: a note reaches the Agent over this socket; its permission mode is the one the
        // note's sender names, so the receiver never holds the note for approval.
        // A socket an earlier run of this Agent left behind is not the new session's to find.
        let socket = self.socket_path(id);
        match std::fs::remove_file(&socket) {
            Err(err) if err.kind() != ErrorKind::NotFound => return Err(RpcError::internal(err)),
            _ => {}
        }
        argv.push("--messaging-socket-path".into());
        argv.push(socket.to_string_lossy().into_owned());
        argv.push("--permission-mode".into());
        argv.push(super::inbox::PERMISSION_MODE.into());
        if let Some(conversation_id) = resume {
            argv.push("--resume".into());
            argv.push(conversation_id.to_owned());
        }
        Ok(argv)
    }

    /// Q11: the production launch seam. Keep this Launcher alive while its Agents run:
    /// it owns their complete bundle. Initial launch, resume and clear use this same seam.
    /// Probe the selected executable once; validate the pinned files before every launch.
    #[allow(clippy::too_many_arguments)]
    pub async fn launch_on(
        &self,
        folders: Folders<'_>,
        id: &str,
        attempt: &str,
        cwd: &Path,
        resume: Option<&str>,
        role: Role,
        pair: &Pair,
    ) -> Result<(Vec<String>, BTreeMap<String, String>), RpcError> {
        let (launcher, project, data, id, attempt, cwd, resume, pair) = (
            self.clone(),
            folders.project.to_owned(),
            folders.data.to_owned(),
            id.to_owned(),
            attempt.to_owned(),
            cwd.to_owned(),
            resume.map(str::to_owned),
            pair.clone(),
        );
        // Trust/config locks and disk delivery cannot hold the async runtime thread.
        let mut env = pair.env();
        let lean = lean_of(role, &pair);
        let (resume_given, id_text) = (resume.is_some(), id.clone());
        // LC2: no memory of the user's other work.
        if lean.is_some() {
            env.insert("CLAUDE_CODE_DISABLE_AUTO_MEMORY".to_owned(), "1".to_owned());
        }
        let bundled = lean.as_ref().is_none_or(|lean| {
            let role = if lean.scout { Role::Scout } else { role };
            profile(role).bundled_plugins
        });
        // I5: every shell the Agent's `claude` runs inherits who it is, and finds `rup` on its PATH.
        env.insert("ROUNDUP_AGENT".to_owned(), id.to_owned());
        if let Some(dir) = self.rup.parent().filter(|dir| dir.is_absolute()) {
            let inherited = std::env::var_os("PATH").unwrap_or_default();
            let paths = std::iter::once(dir.to_path_buf()).chain(std::env::split_paths(&inherited));
            if let Ok(path) = std::env::join_paths(paths) {
                env.insert("PATH".to_owned(), path.to_string_lossy().into_owned());
            }
        }
        let mut argv = tokio::task::spawn_blocking(move || {
            launcher.prepare_on(
                Folders {
                    project: &project,
                    data: &data,
                },
                &id,
                &attempt,
                &cwd,
                resume.as_deref(),
                role,
                &pair,
            )
        })
        .await
        .map_err(RpcError::internal)??;
        let data = folders.data.canonicalize().map_err(RpcError::internal)?;
        let delivery = self
            .plugins
            .get_or_try_init(|| async {
                let dir = data.clone();
                let plugins = tokio::task::spawn_blocking(move || plugins::materialize(&dir))
                    .await
                    .map_err(RpcError::internal)?
                    .map_err(RpcError::internal)?;
                let executable = self.executable()?;
                let runtime = plugins
                    .runtime(&executable)
                    .await
                    .map_err(RpcError::internal)?;
                let question = tempfile::Builder::new()
                    .prefix("q11-plugins-")
                    .tempdir_in(&data)
                    .map_err(RpcError::internal)?;
                for (path, bytes) in QUESTION_FILES {
                    let path = question.path().join(path);
                    std::fs::create_dir_all(path.parent().expect("a plugin asset has a parent"))
                        .map_err(RpcError::internal)?;
                    std::fs::write(path, bytes).map_err(RpcError::internal)?;
                }
                Ok::<_, RpcError>(Delivery {
                    question,
                    data: data.clone(),
                    plugins,
                    runtime,
                })
            })
            .await?;
        if delivery.data != data {
            return Err(RpcError::internal(
                "Q11 Launcher belongs to another Project data directory",
            ));
        }
        let command = delivery
            .runtime
            .command(&delivery.plugins)
            .map_err(RpcError::internal)?;
        for (path, bytes) in QUESTION_FILES {
            if std::fs::read(delivery.question.path().join(path)).map_err(RpcError::internal)?
                != bytes.as_bytes()
            {
                return Err(RpcError::internal(
                    "the native question plugin changed; restart the Daemon",
                ));
            }
        }
        let command = command.as_std();
        argv[0] = command.get_program().to_string_lossy().into_owned();
        // LC5: `hope`, `hunch` and `hold` go to a worker, or to an Agent the user started (LC6).
        if bundled {
            argv.extend(
                command
                    .get_args()
                    .map(|arg| arg.to_string_lossy().into_owned()),
            );
        }
        argv.extend([
            "--plugin-dir".to_owned(),
            delivery.question.path().to_string_lossy().into_owned(),
        ]);
        for (key, value) in command.get_envs() {
            if let Some(value) = value {
                env.insert(
                    key.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                );
            }
        }
        // LC8: the changed Brief has applied once.
        if let Some(mut lean) = lean.filter(|lean| lean.fresh_brief && resume_given) {
            lean.fresh_brief = false;
            Self::save_lean(folders.data, &id_text, &lean)?;
        }
        Ok((argv, env))
    }

    pub(super) fn executable(&self) -> Result<PathBuf, RpcError> {
        let path = Path::new(&self.bin);
        if path.components().count() > 1 || path.is_absolute() {
            return Ok(path.to_owned());
        }
        std::env::var_os("PATH")
            .and_then(|paths| {
                std::env::split_paths(&paths)
                    .map(|dir| dir.join(path))
                    .find(|path| {
                        std::fs::metadata(path).is_ok_and(|meta| {
                            meta.is_file() && meta.permissions().mode() & 0o111 != 0
                        })
                    })
            })
            .ok_or_else(|| {
                RpcError::internal(format!(
                    "Q11 Claude executable {} not found on PATH",
                    self.bin
                ))
            })
    }

    /// Set `hasTrustDialogAccepted` for `cwd`, under its NFC form (the key Claude Code looks up), keeping every other key. A config that cannot be
    /// read as a JSON object is an error, never overwritten. Trust is never withdrawn: an Agent
    /// ending says nothing about the folder, and the user may have trusted it before roundup did.
    fn trust(&self, cwd: &Path) -> Result<(), RpcError> {
        let _serial = TRUSTING.lock().unwrap_or_else(PoisonError::into_inner);
        let _lock = ConfigLock::take(&self.claude_json).map_err(RpcError::internal)?;
        let mut config = match std::fs::read_to_string(&self.claude_json) {
            Ok(text) => serde_json::from_str(&text).map_err(|err| self.unreadable(err))?,
            Err(err) if err.kind() == ErrorKind::NotFound => json!({}),
            Err(err) => return Err(self.unreadable(err)),
        };
        let projects = object(&mut config)
            .and_then(|root| object(root.entry("projects").or_insert_with(|| json!({}))))
            .map_err(|err| self.unreadable(err))?;
        let project = projects
            .entry(cwd.to_string_lossy().nfc().collect::<String>())
            .or_insert_with(|| json!({}));
        object(project)
            .map_err(|err| self.unreadable(err))?
            .insert("hasTrustDialogAccepted".into(), true.into());
        replace_file(
            &landing(&self.claude_json).map_err(RpcError::internal)?,
            &config,
        )
        .map_err(RpcError::internal)
    }

    /// A20: the hook command, not the program, carries the launch's Attempt.
    /// A43: `path` is the Agent's `PATH` in Claude Code's `env`, and `ROUNDUP_AGENT_ID` its id.
    fn settings_json(
        &self,
        id: u64,
        attempt: &str,
        path: &OsStr,
        mcp: bool,
        carried: Option<&Carried>,
    ) -> Value {
        let agent_id = id.to_string();
        let rup = shell_quote(&self.rup.to_string_lossy());
        let id = shell_quote(&id.to_string());
        // H9: `PermissionRequest` waits for the user's answer, so its hook outlives the 5 s of the rest.
        let signal = json!([{ "hooks": [{
            "type": "command",
            "command": format!("ROUNDUP_ATTEMPT={} {rup} signal {id}", shell_quote(attempt)),
            "timeout": 5,
        }] }]);
        let permission = json!([{ "hooks": [{
            "type": "command",
            "command": format!("{rup} permission {id}"),
            "timeout": super::PERMISSION_TIMEOUT_SECS,
        }] }]);
        // E3: at start the Agent is told its Context, by a second command beside the Signal.
        let session_start = json!([{ "hooks": [
            signal[0]["hooks"][0].clone(),
            { "type": "command", "command": format!("{rup} context {id}"), "timeout": 5 },
        ] }]);
        let hooks: Map<String, Value> = STATE_EVENTS
            .iter()
            .map(|event| {
                let hook = match *event {
                    "PermissionRequest" => &permission,
                    "SessionStart" => &session_start,
                    _ => &signal,
                };
                ((*event).to_owned(), hook.clone())
            })
            .collect();
        // Allowed here so a roundup tool never raises a permission dialog.
        let mut settings = json!({ "hooks": hooks });
        if mcp {
            settings["permissions"] = json!({ "allow": ["mcp__roundup__*"] });
        }
        // LC2: the user's permission rules follow roundup's, a rule once.
        if let Some(Carried { allow, deny, login }) = carried {
            for (key, value) in login {
                settings[key] = value.clone();
            }
            let merge = |mut rules: Vec<Value>, more: &[Value]| {
                for rule in more {
                    if !rules.contains(rule) {
                        rules.push(rule.clone());
                    }
                }
                rules
            };
            let own: Vec<Value> = settings["permissions"]["allow"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let allow = merge(own, allow);
            if !allow.is_empty() {
                settings["permissions"]["allow"] = Value::Array(allow);
            }
            let deny = merge(Vec::new(), deny);
            if !deny.is_empty() {
                settings["permissions"]["deny"] = Value::Array(deny);
            }
        }
        // M11: ask for tool search, so the roundup tools' schemas load on demand; the user's own value wins.
        let mut env = Map::new();
        if carried.is_some() {
            env.insert("CLAUDE_CODE_DISABLE_AUTO_MEMORY".into(), "1".into());
        }
        env.insert("CLAUDE_CODE_ENABLE_FUNCTION_HOOKS".into(), "1".into());
        if !self.user_tool_search {
            env.insert("ENABLE_TOOL_SEARCH".into(), "true".into());
        }
        env.insert("ROUNDUP_AGENT_ID".into(), agent_id.into());
        env.insert("PATH".into(), path.to_string_lossy().into());
        settings["env"] = Value::Object(env);
        settings
    }

    /// A43: the folder of `rup`, then (F4) a Door's `shims`, then the `PATH` this process has.
    fn shell_path(&self, shims: Option<&Path>) -> OsString {
        let mut path = self
            .rup
            .parent()
            .map_or_else(OsString::new, |dir| dir.as_os_str().to_owned());
        for next in shims.map(Path::as_os_str).into_iter().chain(
            std::env::var_os("PATH")
                .as_deref()
                .filter(|p| !p.is_empty()),
        ) {
            path.push(":");
            path.push(next);
        }
        path
    }

    /// `RUPD_SOCKET` is left out: Claude Code's environment, which the server inherits, has it.
    fn mcp_servers(&self, id: u64) -> Map<String, Value> {
        let Value::Object(servers) = json!({ "roundup": {
            "type": "stdio",
            "command": self.rup.to_string_lossy(),
            "args": ["mcp", id.to_string()],
        } }) else {
            unreachable!("an object literal");
        };
        servers
    }

    /// LC2: what a lean child takes from the user's settings file; nothing else of it.
    fn user_carried(&self) -> Carried {
        let settings = std::fs::read(self.conversation_dir.join("settings.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .unwrap_or(Value::Null);
        let rules = |key: &str| {
            settings["permissions"][key]
                .as_array()
                .cloned()
                .unwrap_or_default()
        };
        Carried {
            allow: rules("allow"),
            deny: rules("deny"),
            login: LOGIN_KEYS
                .iter()
                .filter_map(|key| Some(((*key).to_owned(), settings.get(key)?.clone())))
                .collect(),
        }
    }

    /// LC5: the user's global `CLAUDE.md`, as read-only text under a heading.
    fn append_global_instructions(&self, brief: &mut String) {
        if let Ok(text) = std::fs::read_to_string(self.conversation_dir.join("CLAUDE.md"))
            && !text.trim().is_empty()
        {
            brief.push_str("\n## The user's global instructions (read-only)\n\n");
            brief.push_str(text.trim_end());
            brief.push('\n');
        }
    }

    /// LC7: a plugin folder of its own whose skills are the extras' skill folders.
    fn extras_plugin(
        &self,
        dir: &Path,
        id: u64,
        skills: &[(String, PathBuf)],
    ) -> Result<PathBuf, RpcError> {
        let folder = extras_dir(dir, id);
        if is_symlink(&folder) {
            return Err(RpcError::internal(format!(
                "{} is a symlink",
                folder.display()
            )));
        }
        match std::fs::remove_dir_all(&folder) {
            Err(err) if err.kind() != ErrorKind::NotFound => return Err(RpcError::internal(err)),
            _ => {}
        }
        replace_text(
            &folder.join(".claude-plugin/plugin.json"),
            r#"{"name":"roundup-extras","version":"0.0.1"}"#,
        )
        .map_err(RpcError::internal)?;
        std::fs::create_dir_all(folder.join("skills")).map_err(RpcError::internal)?;
        for (name, source) in skills {
            std::os::unix::fs::symlink(source, folder.join("skills").join(name))
                .map_err(RpcError::internal)?;
        }
        Ok(folder)
    }

    fn unreadable(&self, err: impl std::fmt::Display) -> RpcError {
        RpcError::internal(format!("{}: {err}", self.claude_json.display()))
    }
}

fn object(value: &mut Value) -> Result<&mut Map<String, Value>, &'static str> {
    value.as_object_mut().ok_or("expected a JSON object")
}

/// An Agent id becomes a file name and a shell word, so only a node id in its one form (decimal,
/// no sign, no leading zero) is accepted; `;` or `../` never reach a shell or a path.
impl Launcher {
    /// The private folder Agents' messaging sockets are made in.
    pub(super) fn socket_dir(&self) -> &Path {
        self.sockets
            .get_or_init(|| socket_folder().expect("a private folder for messaging sockets"))
            .path()
    }
}

/// Claude Code refuses a messaging socket in a folder anyone else can reach.
fn socket_folder() -> std::io::Result<tempfile::TempDir> {
    let folder = tempfile::Builder::new().prefix("rup-").tempdir()?;
    std::fs::set_permissions(folder.path(), std::fs::Permissions::from_mode(0o700))?;
    Ok(folder)
}

fn parse_id(id: &str) -> Option<u64> {
    id.parse()
        .ok()
        .filter(|number: &u64| number.to_string() == id)
}

/// The directory lock Claude Code 2.1.286 holds while it saves its config (`<config>.lock`, as
/// npm's `proper-lockfile` makes it); Claude re-reads the config under it. Released on drop.
struct ConfigLock {
    path: PathBuf,
}

impl ConfigLock {
    /// Wait for the lock, breaking it once stale. An error once it has been held past `STALE`
    /// and then some, which no live holder does.
    fn take(config: &Path) -> std::io::Result<Self> {
        let mut path = config.as_os_str().to_owned();
        path.push(".lock");
        let path = PathBuf::from(path);
        let located = |err: std::io::Error| {
            std::io::Error::new(err.kind(), format!("{}: {err}", path.display()))
        };
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(located)?;
        }
        let deadline = Instant::now() + STALE + Duration::from_secs(1);
        let mut pause = Duration::from_millis(5);
        loop {
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(err) if err.kind() != ErrorKind::AlreadyExists => return Err(located(err)),
                Err(_) if is_stale(&path) => match std::fs::remove_dir(&path) {
                    Err(err) if err.kind() != ErrorKind::NotFound => return Err(located(err)),
                    _ => continue,
                },
                Err(_) if Instant::now() >= deadline => {
                    return Err(std::io::Error::other(format!(
                        "{} is still held after {} s",
                        path.display(),
                        STALE.as_secs() + 1
                    )));
                }
                Err(_) => {}
            }
            std::thread::sleep(pause);
            pause = (pause * 2).min(Duration::from_millis(200));
        }
    }
}

impl Drop for ConfigLock {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_dir(&self.path) {
            eprintln!(
                "agents: could not release {}: {err}; it goes stale in {} s",
                self.path.display(),
                STALE.as_secs()
            );
        }
    }
}

/// A lock whose mtime is in the future (as `proper-lockfile` sets it) is not stale.
fn is_stale(lock: &Path) -> bool {
    std::fs::metadata(lock)
        .and_then(|meta| meta.modified())
        .is_ok_and(|modified| modified.elapsed().is_ok_and(|age| age > STALE))
}

/// One word for a shell, whatever the path holds.
fn shell_quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', r"'\''"))
}

/// Write to a fresh file beside `path`, then rename it over `path` itself, so a reader never sees
/// half a file and two writers never share a temp file. A symlink at `path` is replaced, never
/// written through: the files in the Project are not ours to trust. The old permissions are kept
/// (the config holds credentials).
fn replace_file(path: &Path, value: &Value) -> std::io::Result<()> {
    replace_bytes(path, &serde_json::to_vec_pretty(value)?)
}

/// [`replace_file`] for text.
fn replace_text(path: &Path, text: &str) -> std::io::Result<()> {
    replace_bytes(path, text.as_bytes())
}

fn replace_bytes(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(bytes)?;
    staged.as_file().sync_all()?;
    if let Ok(existing) = std::fs::symlink_metadata(path)
        && existing.is_file()
    {
        staged.as_file().set_permissions(existing.permissions())?;
    }
    staged.persist(path).map_err(|err| err.error)?;
    Ok(())
}

/// The file Claude's config at `path` really is: through any symlinks (a dotfiles manager's), to
/// a target that may not exist yet, so a dangling link stays a link.
fn landing(path: &Path) -> std::io::Result<PathBuf> {
    match path.canonicalize() {
        Err(err) if err.kind() == ErrorKind::NotFound => match std::fs::read_link(path) {
            // A relative target is relative to the link's own folder.
            Ok(next) => landing(&path.parent().unwrap_or(Path::new("")).join(next)),
            // Not a link, or nothing there at all: the write creates `path` itself.
            Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::InvalidInput) => {
                Ok(path.to_owned())
            }
            Err(err) => Err(err),
        },
        found => found,
    }
}

fn agents_dir(dir: &Path) -> PathBuf {
    dir.join("agents")
}

fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

fn settings_path(dir: &Path, id: u64) -> PathBuf {
    agents_dir(dir).join(format!("{id}.settings.json"))
}

fn mcp_config_path(dir: &Path, id: u64) -> PathBuf {
    agents_dir(dir).join(format!("{id}.mcp.json"))
}

fn shim_dir(dir: &Path, id: u64) -> PathBuf {
    agents_dir(dir).join(format!("{id}.bin"))
}

/// The first command-line word of a Door's Claude Code names this file, so a process table row
/// that holds it is that Door's program (F5).
pub fn settings_marker(dir: &Path, id: &str) -> Option<PathBuf> {
    let dir = dir.canonicalize().ok()?;
    Some(settings_path(&dir, parse_id(id)?))
}

/// One executable in `shims` for each program name an Adapter supplies, each failing loud.
fn write_shims(shims: &Path) -> std::io::Result<()> {
    for program in super::programs() {
        let script = format!(
            "#!/bin/sh\necho {} >&2\nexit 1\n",
            shell_quote(SHIM_MESSAGE)
        );
        let path = shims.join(program);
        replace_text(&path, &script)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn remove_shims(dir: &Path, id: u64) -> std::io::Result<()> {
    match std::fs::remove_dir_all(shim_dir(dir, id)) {
        Err(err) if err.kind() != ErrorKind::NotFound => Err(err),
        _ => Ok(()),
    }
}

fn lean_path(dir: &Path, id: u64) -> PathBuf {
    agents_dir(dir).join(format!("{id}.lean.json"))
}

fn extras_dir(dir: &Path, id: u64) -> PathBuf {
    agents_dir(dir).join(format!("{id}.extras"))
}

/// LC1, LC6: how `role`'s child launches: lean for a Door and for the child of a Door or an Agent,
/// the user's full setup for an Agent the user started.
fn lean_of(role: Role, pair: &Pair) -> Option<Lean> {
    match (role, &pair.lean) {
        (_, Some(lean)) => Some(lean.clone()),
        (Role::Door, None) => Some(Lean::default()),
        _ => None,
    }
}

fn brief_path(dir: &Path, id: u64) -> PathBuf {
    agents_dir(dir).join(format!("{id}.brief.md"))
}
