//! What an Agent's Claude Code is started with: a per-Agent settings file whose command hook is
//! `rup hook <agent-id>`, and a pre-trusted working directory (no hook fires for the trust dialog).

use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launcher {
    bin: String,
    /// Claude Code's own config file, where it remembers which folders are trusted.
    claude_json: PathBuf,
}

impl Launcher {
    pub fn new(bin: impl Into<String>, claude_json: PathBuf) -> Self {
        Self {
            bin: bin.into(),
            claude_json,
        }
    }

    /// `ROUNDUP_CLAUDE_BIN` names the program (default `claude`); `CLAUDE_CONFIG_DIR`, else `HOME`,
    /// says where its config lives.
    pub fn from_vars(var: impl Fn(&str) -> Option<OsString>) -> Result<Self, OpenError> {
        let bin = var("ROUNDUP_CLAUDE_BIN")
            .map_or_else(|| "claude".into(), |bin| bin.to_string_lossy().into_owned());
        let claude_json = match (var("CLAUDE_CONFIG_DIR"), var("HOME")) {
            (Some(dir), _) => PathBuf::from(dir).join(".claude.json"),
            (None, Some(home)) => PathBuf::from(home).join(".claude.json"),
            (None, None) => return Err("neither CLAUDE_CONFIG_DIR nor HOME is set".into()),
        };
        Ok(Self::new(bin, claude_json))
    }

    /// Write Agent `id`'s settings under `dir`, trust `cwd`, and return the argv that starts it.
    pub fn prepare(&self, dir: &Path, id: &str, cwd: &Path) -> Result<Vec<String>, RpcError> {
        let cwd = cwd.canonicalize().map_err(|err| {
            RpcError::new(
                code::INVALID_PARAMS,
                format!("cwd {} is unusable: {err}", cwd.display()),
            )
        })?;
        let settings = dir.join("agents").join(format!("{id}.settings.json"));
        write_atomically(&settings, &settings_json(id)).map_err(RpcError::internal)?;
        self.trust(&cwd)?;
        Ok(vec![
            self.bin.clone(),
            "--settings".into(),
            settings.to_string_lossy().into_owned(),
        ])
    }

    /// Set `hasTrustDialogAccepted` for `cwd`, keeping every other key. A config that cannot be
    /// read as a JSON object is an error, never overwritten.
    fn trust(&self, cwd: &Path) -> Result<(), RpcError> {
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

    fn unreadable(&self, err: impl std::fmt::Display) -> RpcError {
        RpcError::internal(format!("{}: {err}", self.claude_json.display()))
    }
}

fn settings_json(id: &str) -> Value {
    let hook = json!([{ "hooks": [{ "type": "command", "command": format!("rup hook {id}") }] }]);
    let hooks: Map<String, Value> = STATE_EVENTS
        .iter()
        .map(|event| ((*event).to_owned(), hook.clone()))
        .collect();
    json!({ "hooks": hooks })
}

fn object(value: &mut Value) -> Result<&mut Map<String, Value>, &'static str> {
    value.as_object_mut().ok_or("expected a JSON object")
}

/// Write beside `path`, then rename over it, keeping the old file's permissions (the config holds
/// credentials), so a reader never sees half a file.
fn write_atomically(path: &Path, value: &Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut staged = path.as_os_str().to_owned();
    staged.push(".roundup-tmp");
    let staged = PathBuf::from(staged);
    std::fs::write(&staged, serde_json::to_vec_pretty(value)?)?;
    if let Ok(existing) = std::fs::metadata(path) {
        std::fs::set_permissions(&staged, existing.permissions())?;
    }
    std::fs::rename(&staged, path)
}
