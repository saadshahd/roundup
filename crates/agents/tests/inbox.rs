//! B50, B51: the adapter's side of Claude Code's messaging socket, against the recorded fixtures
//! of `fixtures/inbox/<version>/`. Re-record them from the real Claude Code when its version
//! changes; the real round trip is proved by the e2e tests that run the pinned `claude`.

use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use agents::claude_code::Launcher;
use agents::claude_code::inbox::{Entry, recorded};
use contracts::agent::Inbox;
use serde_json::Value;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/inbox/2.1.300");

/// A Launcher whose `claude` prints `version`, and the config folder it looks for sessions in.
fn launcher(version: &str) -> (Launcher, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("claude");
    std::fs::write(&bin, format!("#!/bin/sh\necho '{version} (Claude Code)'\n")).unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let launcher = Launcher::new(
        bin.to_string_lossy(),
        dir.path().join("claude.json"),
        PathBuf::from("rup"),
        None,
    );
    (launcher, dir)
}

/// What Claude Code does for Agent `a` at launch: listens on its socket and lists the session
/// with its token, as the recorded `session.json` and `session.key` do.
fn session(launcher: &Launcher, config: &Path) -> UnixListener {
    listed(launcher, config, std::process::id())
}

/// `session`, listed under `pid`.
fn listed(launcher: &Launcher, config: &Path, pid: u32) -> UnixListener {
    let socket = launcher.socket_path("a");
    let listener = UnixListener::bind(&socket).unwrap();
    let sessions = config.join("sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    let json = std::fs::read_to_string(Path::new(FIXTURES).join("session.json")).unwrap();
    std::fs::write(
        sessions.join("69890.json"),
        json.replace("SOCKET", &socket.to_string_lossy())
            .replace("PID", &pid.to_string()),
    )
    .unwrap();
    std::fs::copy(
        Path::new(FIXTURES).join("session.key"),
        sessions.join("69890.78f995701ed0.key"),
    )
    .unwrap();
    listener
}

fn note(id: u32, text: &str, one_line: bool) -> Entry {
    Entry {
        id,
        from: "rupd".into(),
        text: text.into(),
        one_line,
    }
}

/// The lines one connection sends, as JSON.
fn lines(listener: &UnixListener) -> Vec<Value> {
    let (stream, _) = listener.accept().unwrap();
    BufReader::new(stream)
        .lines()
        .map(|line| serde_json::from_str(&line.unwrap()).unwrap())
        .collect()
}

#[tokio::test]
async fn b51_a_killed_session_left_listed_is_missing() {
    let (launcher, dir) = launcher("2.1.300");
    let mut gone = std::process::Command::new("true").spawn().unwrap();
    let pid = gone.id();
    gone.wait().unwrap();
    let _listener = listed(&launcher, dir.path(), pid);
    assert_eq!(launcher.inbox_check("a").await, Inbox::Missing);
}

#[tokio::test]
async fn b50_a_note_is_an_auth_line_then_one_labelled_line_in_the_launch_permission_mode() {
    let (launcher, dir) = launcher("2.1.300");
    let listener = session(&launcher, dir.path());
    launcher
        .inbox_write("a", &[note(7, "look", false), note(8, "again", false)])
        .unwrap();
    let sent = lines(&listener);
    assert_eq!(sent[0]["type"], "auth");
    assert_eq!(sent[0]["token"], "f68a2cb41ca29945a351924446fcbf1f");
    assert_eq!(sent.len(), 3, "one line per note");
    assert_eq!(sent[1]["msg_id"], "00000000-0000-4000-8000-000000000007");
    assert_eq!(sent[2]["msg_id"], "00000000-0000-4000-8000-000000000008");
    let content = sent[1]["message"]["content"].as_str().unwrap();
    assert!(
        content.starts_with("<cross-session-message from=\"uds:")
            && content.contains(
                "from-name=\"rupd\" from-mode=\"default\">\nlook\n</cross-session-message>"
            ),
        "{content}"
    );
    assert_eq!(
        (&sent[1]["type"], &sent[1]["priority"]),
        (&"user".into(), &"next".into())
    );
}

#[tokio::test]
async fn b50_a_long_note_shows_as_one_line() {
    let (launcher, dir) = launcher("2.1.300");
    let listener = session(&launcher, dir.path());
    let long = format!("[digest] {}", "é".repeat(300));
    launcher
        .inbox_write("a", &[note(1, &long, true), note(2, &long, false)])
        .unwrap();
    let sent = lines(&listener);
    let body = |line: &Value| {
        let content = line["message"]["content"].as_str().unwrap();
        content.split('\n').nth(1).unwrap().to_owned()
    };
    let (cut, whole) = (body(&sent[1]), body(&sent[2]));
    assert!(cut.len() <= 200 && cut.ends_with("[cut]"), "{cut}");
    assert_eq!(whole, long, "only a [wake] or [digest] note is cut");
}

#[test]
fn b50_the_transcript_records_a_note_as_a_peer_message_under_its_msg_id() {
    let transcript = Path::new(FIXTURES).join("transcript.jsonl");
    // The user's prompt carries no origin; a note carries the msg_id it was sent under, as a user
    // entry between turns (7) and as a queued attachment mid-turn (11).
    assert_eq!(recorded(&transcript, 0).0, [7, 11]);
}

#[test]
fn b50_a_transcript_is_read_from_where_it_was_left() {
    let transcript = tempfile::NamedTempFile::new().unwrap();
    let line = |id: u32| {
        format!(
            "{{\"origin\":{{\"kind\":\"peer\",\"msg_id\":\"00000000-0000-4000-8000-{id:012x}\"}}}}\n"
        )
    };
    std::fs::write(
        transcript.path(),
        format!("{}{}", line(1), line(2).trim_end()),
    )
    .unwrap();
    let (ids, offset) = recorded(transcript.path(), 0);
    assert_eq!(ids, [1], "a line not yet complete waits");
    std::fs::write(
        transcript.path(),
        format!("{}{}{}", line(1), line(2), line(3)),
    )
    .unwrap();
    assert_eq!(recorded(transcript.path(), offset).0, [2, 3]);
}
