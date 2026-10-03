//! U104 the Daemon holds the screen: `terminal.snapshot {id}` and `offset` on `terminal.output`.
//! Written by an architect before the Builder; the Builder makes these pass and never edits them.
//! Every test replays bytes into a second `vt100` parser, the way the webview's emulator would.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use contracts::EventData;
use contracts::terminal::SpawnParams;
use rpc::{Module, code};
use serde_json::{Value, json};
use tokio::sync::broadcast::Receiver;

use crate::common::{PATIENCE, ctx, open, sh};

const SCROLLBACK: usize = 1000;
const MIB: usize = 1 << 20;

struct Chunk {
    offset: u64,
    bytes: Vec<u8>,
}

struct Snapshot {
    cols: u16,
    rows: u16,
    after: u64,
    data: Vec<u8>,
}

async fn snapshot(terminals: &terminal::Terminals, dir: &tempfile::TempDir, id: &str) -> Snapshot {
    let wire = terminals
        .call(
            &ctx(dir, &rpc::Bus::new()),
            "terminal.snapshot",
            json!({"id": id}),
        )
        .await
        .expect("terminal.snapshot answers");
    Snapshot {
        cols: wire["cols"].as_u64().unwrap().try_into().unwrap(),
        rows: wire["rows"].as_u64().unwrap().try_into().unwrap(),
        after: wire["after"].as_u64().unwrap(),
        data: STANDARD.decode(wire["data"].as_str().unwrap()).unwrap(),
    }
}

fn chunk(out: &contracts::terminal::OutputEvent) -> Chunk {
    let wire = serde_json::to_value(out).unwrap();
    Chunk {
        offset: wire["offset"]
            .as_u64()
            .expect("terminal.output carries an offset"),
        bytes: STANDARD.decode(&out.data).unwrap(),
    }
}

/// Every output chunk until the program exits.
async fn output_until_exit(events: &mut Receiver<EventData>) -> Vec<Chunk> {
    let mut chunks = vec![];
    tokio::time::timeout(PATIENCE, async {
        loop {
            match events.recv().await.expect("events stay open") {
                EventData::TerminalOutput(out) => chunks.push(chunk(&out)),
                EventData::TerminalExited(_) => return,
                _ => {}
            }
        }
    })
    .await
    .expect("the program exits in time");
    chunks
}

async fn run(script: &str) -> (tempfile::TempDir, terminal::Terminals, String, Vec<Chunk>) {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut spawned = terminals.spawn(sh(dir.path(), script)).await.unwrap();
    let chunks = output_until_exit(&mut spawned.events).await;
    (dir, terminals, spawned.id, chunks)
}

fn parser(rows: u16, cols: u16, bytes: impl IntoIterator<Item = u8>) -> vt100::Parser {
    let mut parser = vt100::Parser::new(rows, cols, SCROLLBACK);
    parser.process(&bytes.into_iter().collect::<Vec<_>>());
    parser
}

/// What a user could see or rely on: screen with attributes, cursor, modes, scrollback text.
fn looks(parser: &mut vt100::Parser) -> Value {
    let screen = parser.screen_mut();
    screen.set_scrollback(usize::MAX);
    let history = screen.contents();
    screen.set_scrollback(0);
    json!({
        "screen": STANDARD.encode(screen.contents_formatted()),
        "cursor": screen.cursor_position(),
        "hidden": screen.hide_cursor(),
        "alternate": screen.alternate_screen(),
        "paste": screen.bracketed_paste(),
        "app_cursor": screen.application_cursor(),
        "mouse": format!("{:?}", screen.mouse_protocol_mode()),
        "history": history,
    })
}

fn all_bytes(chunks: &[Chunk]) -> impl Iterator<Item = u8> + '_ {
    chunks.iter().flat_map(|c| c.bytes.iter().copied())
}

#[tokio::test]
async fn u104_a_snapshot_reproduces_screen_cursor_modes_and_scrollback() {
    let script = r"printf '\033[?2004h\033[?1h\033[?25l\033[31mred\033[0m \033[1;44mbold\033[0m\n'; \
        i=0; while [ $i -lt 1200 ]; do echo line$i; i=$((i+1)); done; printf '\033[5;10Hhere'";
    let (dir, terminals, id, chunks) = run(script).await;
    let snap = snapshot(&terminals, &dir, &id).await;

    let mut expected = parser(snap.rows, snap.cols, all_bytes(&chunks));
    let mut restored = parser(snap.rows, snap.cols, snap.data);

    assert_eq!(looks(&mut restored), looks(&mut expected));
}

#[tokio::test]
async fn u104_a_snapshot_keeps_the_alternate_screen() {
    let (dir, terminals, id, chunks) =
        run(r"printf 'main\n\033[?1049h\033[2J\033[3;4Halt text'").await;
    let snap = snapshot(&terminals, &dir, &id).await;

    let mut expected = parser(snap.rows, snap.cols, all_bytes(&chunks));
    let mut restored = parser(snap.rows, snap.cols, snap.data);

    assert!(looks(&mut restored)["alternate"].as_bool().unwrap());
    assert_eq!(looks(&mut restored), looks(&mut expected));
}

#[tokio::test]
async fn u104_data_is_at_most_one_mib_and_the_visible_screen_is_never_cut() {
    let script = r"i=0; while [ $i -lt 4000 ]; do printf '%0400d\n' $i; i=$((i+1)); done; printf 'LAST LINE'";
    let (dir, terminals, id, chunks) = run(script).await;
    let snap = snapshot(&terminals, &dir, &id).await;

    let visible = |bytes: Vec<u8>| {
        let parser = parser(snap.rows, snap.cols, bytes);
        parser.screen().contents_formatted()
    };

    assert!(snap.data.len() <= MIB, "{} bytes", snap.data.len());
    assert_eq!(
        visible(snap.data.clone()),
        visible(all_bytes(&chunks).collect())
    );
}

#[tokio::test]
async fn u104_offsets_count_the_bytes_before_each_event_across_the_terminals_life() {
    let (dir, terminals, id, chunks) =
        run(r"echo one; sleep 0.2; echo two; sleep 0.2; echo three").await;
    let snap = snapshot(&terminals, &dir, &id).await;

    let mut next = 0;
    for chunk in &chunks {
        assert_eq!(chunk.offset, next);
        next += chunk.bytes.len() as u64;
    }

    assert!(chunks.len() > 1);
    assert_eq!(snap.after, next);
}

#[tokio::test]
async fn u104_a_snapshot_taken_during_a_burst_splits_the_events_exactly_at_after() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let script = r"i=0; while [ $i -lt 20000 ]; do echo burst-line-$i; i=$((i+1)); done";
    let mut spawned = terminals.spawn(sh(dir.path(), script)).await.unwrap();
    let mut chunks = vec![];
    let mut mid = None;
    tokio::time::timeout(PATIENCE, async {
        loop {
            match spawned.events.recv().await.expect("events stay open") {
                EventData::TerminalOutput(out) => {
                    chunks.push(chunk(&out));
                    if mid.is_none() && chunks.len() == 3 {
                        mid = Some(snapshot(&terminals, &dir, &spawned.id).await);
                    }
                }
                EventData::TerminalExited(_) => return,
                _ => {}
            }
        }
    })
    .await
    .expect("the program exits in time");
    let snap = mid.expect("the burst has three events");

    let held = chunks.iter().filter_map(|c| {
        let end = c.offset + c.bytes.len() as u64;
        let skip = snap.after.saturating_sub(c.offset) as usize;
        (end > snap.after).then(|| c.bytes[skip..].to_vec())
    });
    let mut restored = parser(
        snap.rows,
        snap.cols,
        snap.data.iter().copied().chain(held.flatten()),
    );
    let mut expected = parser(snap.rows, snap.cols, all_bytes(&chunks));

    assert_eq!(looks(&mut restored), looks(&mut expected));
}

#[tokio::test]
async fn u104_a_snapshot_has_the_size_after_the_last_resize() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let spawned = terminals.spawn(sh(dir.path(), "sleep 30")).await.unwrap();

    let before = snapshot(&terminals, &dir, &spawned.id).await;
    terminals.resize(&spawned.id, 100, 30).await.unwrap();
    let after = snapshot(&terminals, &dir, &spawned.id).await;

    assert_eq!((before.cols, before.rows), (80, 24));
    assert_eq!((after.cols, after.rows), (100, 30));
    terminals.kill(&spawned.id).await.unwrap();
}

#[tokio::test]
async fn u104_an_unknown_id_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, bus) = open(&dir);

    let err = terminals
        .call(&ctx(&dir, &bus), "terminal.snapshot", json!({"id": "nope"}))
        .await
        .unwrap_err();

    assert_eq!(err.code, code::NOT_FOUND);
}

#[tokio::test]
async fn u104_exited_terminal_snapshots_its_last_screen() {
    let (dir, terminals, id, _) = run(r"printf 'goodbye\n'").await;

    let snap = snapshot(&terminals, &dir, &id).await;

    let restored = parser(snap.rows, snap.cols, snap.data);
    assert!(restored.screen().contents().contains("goodbye"));
}

#[tokio::test]
async fn u104_snapshot_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut params: SpawnParams = sh(dir.path(), r"printf 'steady\n'; sleep 30");
    (params.cols, params.rows) = (90, 20);
    let mut spawned = terminals.spawn(params).await.unwrap();
    crate::common::until_printed(&mut spawned.events, "steady").await;
    let listed = serde_json::to_value(terminals.list()).unwrap();

    let first = snapshot(&terminals, &dir, &spawned.id).await;
    let second = snapshot(&terminals, &dir, &spawned.id).await;

    assert_eq!(
        (first.cols, first.rows, first.after, &first.data),
        (second.cols, second.rows, second.after, &second.data)
    );
    assert_eq!(serde_json::to_value(terminals.list()).unwrap(), listed);
    terminals.kill(&spawned.id).await.unwrap();
}
