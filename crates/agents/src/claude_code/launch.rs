//! What an Agent's Claude Code is started with: a per-Agent settings file whose command hook is
//! `rup signal <agent-id>`, a per-Agent MCP config that runs `rup mcp <agent-id>`, and a pre-trusted
//! working directory (no hook fires for the trust dialog).

use std::ffi::OsString;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rpc::{OpenError, RpcError, code};
use serde_json::{Map, Value, json};

/// The hook events that carry state. Notification and SubagentStop are left out on purpose: the
/// first arrives about 6 s late, the second fires spuriously (ADR 0006).
const STATE_EVENTS: [&str; 9] = [
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launcher {
    bin: String,
    /// Claude Code's own config file, where it remembers which folders are trusted.
    claude_json: PathBuf,
    /// The `rup` the hooks run, by absolute path: a hook's PATH is not ours to trust.
    rup: PathBuf,
}

impl Launcher {
    pub fn new(bin: impl Into<String>, claude_json: PathBuf, rup: PathBuf) -> Self {
        Self {
            bin: bin.into(),
            claude_json,
            rup,
        }
    }

    pub fn from_env() -> Result<Self, OpenError> {
        Self::from_vars(|name| std::env::var_os(name), &std::env::current_exe()?)
    }

    /// `ROUNDUP_CLAUDE_BIN` names the program (default `claude`); `CLAUDE_CONFIG_DIR`, else `HOME`,
    /// says where its config lives; `ROUNDUP_RUP_BIN`, else the `rup` beside `exe` (the running
    /// Daemon), is what the hooks run.
    pub fn from_vars(
        var: impl Fn(&str) -> Option<OsString>,
        exe: &Path,
    ) -> Result<Self, OpenError> {
        let bin = var("ROUNDUP_CLAUDE_BIN")
            .map_or_else(|| "claude".into(), |bin| bin.to_string_lossy().into_owned());
        let claude_json = match (var("CLAUDE_CONFIG_DIR"), var("HOME")) {
            (Some(dir), _) => PathBuf::from(dir).join(".claude.json"),
            (None, Some(home)) => PathBuf::from(home).join(".claude.json"),
            (None, None) => return Err("neither CLAUDE_CONFIG_DIR nor HOME is set".into()),
        };
        let rup = var("ROUNDUP_RUP_BIN").map_or_else(|| exe.with_file_name("rup"), PathBuf::from);
        Ok(Self::new(bin, claude_json, rup))
    }

    /// Delete what `prepare` wrote for Agent `id`, for an Agent that never started.
    pub fn discard(dir: &Path, id: &str) -> std::io::Result<()> {
        // `prepare` writes nothing for an id that is not a node id.
        let Some(id) = parse_id(id) else {
            return Ok(());
        };
        for file in [settings_path(dir, id), mcp_config_path(dir, id)] {
            match std::fs::remove_file(file) {
                Err(err) if err.kind() != ErrorKind::NotFound => return Err(err),
                _ => {}
            }
        }
        Ok(())
    }

    /// Write Agent `id`'s settings and MCP config under `dir` (the Project's `.roundup/`), trust
    /// `cwd`, and return the argv that starts it. Only a `cwd` inside the Project folder is trusted: Claude's config
    /// is the user's, and this is the only grant roundup makes in it.
    pub fn prepare(&self, dir: &Path, id: &str, cwd: &Path) -> Result<Vec<String>, RpcError> {
        let invalid = |message: String| RpcError::new(code::INVALID_PARAMS, message);
        let id = parse_id(id).ok_or_else(|| invalid(format!("{id:?} is not a node id")))?;
        let cwd = cwd
            .canonicalize()
            .map_err(|err| invalid(format!("cwd {} is unusable: {err}", cwd.display())))?;
        if !cwd.is_dir() {
            return Err(invalid(format!("cwd {} is not a folder", cwd.display())));
        }
        // Claude reads the settings path from its own cwd, so it must be absolute.
        let dir = dir
            .canonicalize()
            .map_err(|err| RpcError::internal(format!("{}: {err}", dir.display())))?;
        let project = dir.parent().ok_or_else(|| {
            RpcError::internal(format!("{} has no project folder", dir.display()))
        })?;
        if !cwd.starts_with(project) {
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
        let settings = settings_path(&dir, id);
        let mcp_config = mcp_config_path(&dir, id);
        write_atomically(&settings, &self.settings_json(id)).map_err(RpcError::internal)?;
        write_atomically(&mcp_config, &self.mcp_json(id)).map_err(RpcError::internal)?;
        self.trust(&cwd)?;
        // No `--strict-mcp-config`: the Agent keeps the user's own servers.
        Ok(vec![
            self.bin.clone(),
            "--settings".into(),
            settings.to_string_lossy().into_owned(),
            "--mcp-config".into(),
            mcp_config.to_string_lossy().into_owned(),
        ])
    }

    /// Set `hasTrustDialogAccepted` for `cwd`, keeping every other key. A config that cannot be
    /// read as a JSON object is an error, never overwritten. Trust is never withdrawn: an Agent
    /// ending says nothing about the folder, and the user may have trusted it before roundup did.
    fn trust(&self, cwd: &Path) -> Result<(), RpcError> {
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
            .entry(cwd.to_string_lossy().into_owned())
            .or_insert_with(|| json!({}));
        object(project)
            .map_err(|err| self.unreadable(err))?
            .insert("hasTrustDialogAccepted".into(), true.into());
        write_atomically(&self.claude_json, &config).map_err(RpcError::internal)
    }

    fn settings_json(&self, id: u64) -> Value {
        let command = format!(
            "{} signal {}",
            shell_quote(&self.rup.to_string_lossy()),
            shell_quote(&id.to_string())
        );
        let hook = json!([{ "hooks": [{ "type": "command", "command": command, "timeout": 5 }] }]);
        let hooks: Map<String, Value> = STATE_EVENTS
            .iter()
            .map(|event| ((*event).to_owned(), hook.clone()))
            .collect();
        // Allowed here so a roundup tool never raises a permission dialog.
        json!({ "hooks": hooks, "permissions": { "allow": ["mcp__roundup__*"] } })
    }

    /// `RUPD_SOCKET` is left out: Claude Code's environment, which the server inherits, has it.
    fn mcp_json(&self, id: u64) -> Value {
        json!({ "mcpServers": { "roundup": {
            "type": "stdio",
            "command": self.rup.to_string_lossy(),
            "args": ["mcp", id.to_string()],
        } } })
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

/// Write to a fresh file beside `path`, then rename it over, so a reader never sees half a file
/// and two writers never share a temp file. A symlink is written through and stays a symlink; the
/// old permissions are kept (the config holds credentials).
fn write_atomically(path: &Path, value: &Value) -> std::io::Result<()> {
    let target = landing(path)?;
    let parent = target.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(&serde_json::to_vec_pretty(value)?)?;
    staged.as_file().sync_all()?;
    if let Ok(existing) = std::fs::metadata(&target) {
        staged.as_file().set_permissions(existing.permissions())?;
    }
    staged.persist(&target).map_err(|err| err.error)?;
    Ok(())
}

/// The file a write to `path` replaces: through any symlinks, to a target that may not exist yet,
/// so a dangling link stays a link.
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

fn settings_path(dir: &Path, id: u64) -> PathBuf {
    dir.join("agents").join(format!("{id}.settings.json"))
}

fn mcp_config_path(dir: &Path, id: u64) -> PathBuf {
    dir.join("agents").join(format!("{id}.mcp.json"))
}
