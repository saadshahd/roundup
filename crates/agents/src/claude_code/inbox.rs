//! B50, B51: Claude Code's cross-session inbox, the messaging socket each Agent is launched with
//! (`--messaging-socket-path`). A `note` is one line of NDJSON written to it, after an `auth` line
//! with the session's `peerToken`; the transcript records it with an `origin` of kind `peer` under the
//! `msg_id` sent. The wire format is the one probed on the versions in `SUPPORTED`.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use contracts::agent::Inbox;
use serde_json::{Value, json};

use super::Launcher;

/// The versions of the installed Claude Code this adapter has probed the messaging socket on.
const SUPPORTED: [&str; 1] = ["2.1.300"];

/// The permission mode an Agent is launched in, and the `from-mode` a note names: a session whose
/// mode differs from its sender's holds the note for approval.
pub(super) const PERMISSION_MODE: &str = "default";

/// How long `claude --version` may take before the version counts as unknown.
const VERSION_BOUND: Duration = Duration::from_secs(5);

/// How long a write to an Agent's socket may block.
const WRITE_BOUND: Duration = Duration::from_secs(2);

/// B50(d): the longest entry text one line of the TUI shows, `[cut]` included.
const LINE_BYTES: usize = 200;
const CUT: &str = "[cut]";

/// One note for an Agent: `id` is the Message's, `from` the sender's name. `one_line` asks for
/// the text to be cut to one line of the TUI (B50(d)).
pub struct Entry {
    pub id: u32,
    pub from: String,
    pub text: String,
    pub one_line: bool,
}

const MSG_ID_PREFIX: &str = "00000000-0000-4000-8000-";

/// The `msg_id` of Message `id`: Claude Code wants a UUID and drops a second line carrying one it
/// has taken up, so a note sent twice reaches the Agent once. `message_of` is its inverse.
fn msg_id(id: u32) -> String {
    format!("{MSG_ID_PREFIX}{id:012x}")
}

fn message_of(msg_id: &str) -> Option<u32> {
    u32::from_str_radix(msg_id.strip_prefix(MSG_ID_PREFIX)?, 16).ok()
}

/// B50(d): `text` as one line: cut at a character boundary so the whole, `[cut]` included, is at
/// most 200 bytes, when it is longer.
fn one_line(text: &str) -> String {
    if text.len() <= LINE_BYTES {
        return text.to_owned();
    }
    let mut end = LINE_BYTES - CUT.len();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{CUT}", &text[..end])
}

/// The sender's name as an attribute value of the `cross-session-message` tag, without the
/// characters that would end the attribute or the tag.
fn attribute(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '"' | '<' | '>' | '\n' | '\r' => '_',
            c => c,
        })
        .collect()
}

impl Launcher {
    /// The socket Agent `agent` is launched to make.
    pub fn socket_path(&self, agent: impl std::fmt::Display) -> PathBuf {
        self.socket_dir().join(format!("{agent}.sock"))
    }

    /// The installed Claude Code's version, from `claude --version`; `None` when it does not
    /// answer in time with a version. A version found once is kept.
    async fn version(&self) -> Option<String> {
        self.version
            .get_or_try_init(|| async { self.ask_version().await.ok_or(()) })
            .await
            .ok()
            .cloned()
    }

    async fn ask_version(&self) -> Option<String> {
        // A launcher script finds its install beside itself, so it runs from where a link leads.
        let bin = self.executable().ok()?.canonicalize().ok()?;
        let run = tokio::process::Command::new(bin)
            .arg("--version")
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .output();
        let out = tokio::time::timeout(VERSION_BOUND, run)
            .await
            .ok()?
            .ok()
            .filter(|out| out.status.success())?;
        let text = String::from_utf8(out.stdout).ok()?;
        let word = text.split_whitespace().next()?;
        word.split('.')
            .all(|part| part.parse::<u32>().is_ok())
            .then(|| word.to_owned())
    }

    /// The `peerToken` of the live session listening on `socket`: Claude Code lists each session
    /// in `<config>/sessions/<pid>.json` and keeps its token in a `<pid>.<hash>.key` beside it. A
    /// session that was killed leaves its files, so only a listed process still running counts.
    fn peer_token(&self, socket: &Path) -> Option<String> {
        let socket = socket.canonicalize().ok()?;
        let sessions = self.conversation_dir.join("sessions");
        let files: Vec<_> = std::fs::read_dir(&sessions).ok()?.flatten().collect();
        let pid = files.iter().find_map(|file| {
            let name = file.file_name().into_string().ok()?;
            let pid = name.strip_suffix(".json")?;
            let session: Value = serde_json::from_slice(&std::fs::read(file.path()).ok()?).ok()?;
            let listening = Path::new(session["messagingSocketPath"].as_str()?)
                .canonicalize()
                .ok()?;
            let alive = rustix::process::Pid::from_raw(session["pid"].as_i64()?.try_into().ok()?)
                .is_some_and(|pid| rustix::process::test_kill_process(pid).is_ok());
            (listening == socket && alive).then(|| pid.to_owned())
        })?;
        files.iter().find_map(|file| {
            let name = file.file_name().into_string().ok()?;
            name.strip_prefix(&format!("{pid}."))?
                .strip_suffix(".key")?;
            let key: Value = serde_json::from_slice(&std::fs::read(file.path()).ok()?).ok()?;
            key["peerToken"].as_str().map(str::to_owned)
        })
    }

    /// B51: the state of Agent `agent`'s inbox: `missing` when its session is not listening on
    /// its socket; `mismatch` when the installed Claude Code has not been probed here.
    pub async fn inbox_check(&self, agent: &str) -> Inbox {
        if self.peer_token(&self.socket_path(agent)).is_none() {
            return Inbox::Missing;
        }
        match self.version().await {
            Some(version) if SUPPORTED.contains(&version.as_str()) => Inbox::Ok,
            _ => Inbox::Mismatch,
        }
    }

    /// B50(a), B50(c): sends one line per note over `agent`'s socket. A failure, which may come
    /// after some notes went, names `missing`; sending the batch again is safe, since Claude Code
    /// drops a `msg_id` it has taken up.
    pub fn inbox_write(&self, agent: &str, notes: &[Entry]) -> Result<(), Inbox> {
        let socket = self.socket_path(agent);
        let token = self.peer_token(&socket).ok_or(Inbox::Missing)?;
        let mut stream = UnixStream::connect(&socket).map_err(|_| Inbox::Missing)?;
        stream
            .set_write_timeout(Some(WRITE_BOUND))
            .map_err(|_| Inbox::Missing)?;
        let from = format!("uds:{}", self.socket_dir().join("rupd").display());
        let mut lines = vec![json!({ "type": "auth", "token": token })];
        for note in notes {
            let text = if note.one_line {
                one_line(&note.text)
            } else {
                note.text.clone()
            };
            let content = format!(
                "<cross-session-message from=\"{from}\" from-name=\"{}\" from-mode=\"{PERMISSION_MODE}\">\n{text}\n</cross-session-message>",
                attribute(&note.from),
            );
            lines.push(json!({
                "msgV": 1, "msg_id": msg_id(note.id), "type": "user",
                "message": { "role": "user", "content": content },
                "priority": "next", "from": from,
            }));
        }
        let mut body = Vec::new();
        for line in &lines {
            serde_json::to_writer(&mut body, line).map_err(|_| Inbox::Missing)?;
            body.push(b'\n');
        }
        stream.write_all(&body).map_err(|_| Inbox::Missing)
    }
}

/// The Message ids the transcript at `path` records as another session's message, from byte
/// `from` on, and the offset to continue from: the end of the last complete line.
pub fn recorded(path: &Path, from: u64) -> (Vec<u32>, u64) {
    let Ok(bytes) = std::fs::read(path) else {
        return (Vec::new(), from);
    };
    let start = usize::try_from(from).unwrap_or(usize::MAX).min(bytes.len());
    let tail = &bytes[start..];
    let end = tail
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |end| end + 1);
    let ids = tail[..end]
        .split(|byte| *byte == b'\n')
        .filter_map(|line| serde_json::from_slice::<Value>(line).ok())
        .filter_map(|record| {
            // A message taken up between turns is a `user` entry; one that arrives mid-turn is
            // folded into the turn as a `queued_command` attachment. Both carry the `origin`.
            let origin = if record["origin"].is_object() {
                &record["origin"]
            } else {
                &record["attachment"]["origin"]
            };
            (origin["kind"] == "peer")
                .then(|| message_of(origin["msg_id"].as_str()?))
                .flatten()
        })
        .collect();
    (ids, (start + end) as u64)
}

/// The transcript file a hook payload names.
pub fn transcript_of(payload: &Value) -> Option<PathBuf> {
    payload["transcript_path"].as_str().map(PathBuf::from)
}
