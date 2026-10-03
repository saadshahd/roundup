use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use contracts::EventData;
use contracts::terminal::{Snapshot, TerminalId};
use rpc::{Module, code};

use crate::common::{ctx, open, sh, until_exit, until_printed};

#[tokio::test]
async fn u104_snapshot_restores_the_visible_screen_and_input_modes() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut spawned = terminals
        .spawn(sh(
            dir.path(),
            r"printf '\033[?1049h\033[?2004h\033[?1h\033[?1000h\033[31mred\033[2;3H@'; read line",
        ))
        .await
        .unwrap();
    until_printed(&mut spawned.events, "@").await;
    let snapshot = terminals.snapshot(&spawned.id).unwrap();
    let mut restored = vt100::Parser::new(snapshot.rows, snapshot.cols, 1000);
    restored.process(&STANDARD.decode(snapshot.data).unwrap());
    let screen = restored.screen();
    assert!(screen.contents().contains("red"));
    assert_eq!(screen.cell(0, 0).unwrap().fgcolor(), vt100::Color::Idx(1));
    assert_eq!(screen.cursor_position(), (1, 3));
    assert!(screen.alternate_screen());
    assert!(screen.bracketed_paste());
    assert!(screen.application_cursor());
    assert_eq!(
        screen.mouse_protocol_mode(),
        vt100::MouseProtocolMode::PressRelease
    );
    terminals.kill(&spawned.id).await.unwrap();
}

#[tokio::test]
async fn u104_snapshot_of_an_exited_terminal_keeps_its_last_screen() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut spawned = terminals
        .spawn(sh(dir.path(), "printf 'last-screen' "))
        .await
        .unwrap();
    until_exit(&mut spawned.events).await;
    let snapshot = terminals.snapshot(&spawned.id).unwrap();
    let mut restored = vt100::Parser::new(snapshot.rows, snapshot.cols, 1000);
    restored.process(&STANDARD.decode(snapshot.data).unwrap());
    assert!(restored.screen().contents().contains("last-screen"));
}

#[tokio::test]
async fn u104_snapshot_of_an_unknown_terminal_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    assert_eq!(terminals.snapshot("99").unwrap_err().code, code::NOT_FOUND);
}

#[tokio::test]
async fn u104_idle_terminal_starts_with_a_blank_snapshot_then_tracks_output() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut spawned = terminals
        .spawn(sh(dir.path(), r#"read line; printf '%s' "$line""#))
        .await
        .unwrap();
    let blank = terminals.snapshot(&spawned.id).unwrap();
    assert_eq!((blank.cols, blank.rows, blank.after), (80, 24, 0));
    terminals.write(&spawned.id, b"hello\n").await.unwrap();
    until_printed(&mut spawned.events, "hello").await;
    assert!(terminals.snapshot(&spawned.id).unwrap().after > blank.after);
    until_exit(&mut spawned.events).await;
}

#[tokio::test]
async fn u104_snapshot_is_available_over_rpc() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, bus) = open(&dir);
    let mut spawned = terminals
        .spawn(sh(dir.path(), "printf 'rpc-screen'"))
        .await
        .unwrap();
    until_exit(&mut spawned.events).await;
    let value = terminals
        .call(
            &ctx(&dir, &bus),
            "terminal.snapshot",
            serde_json::to_value(TerminalId { id: spawned.id }).unwrap(),
        )
        .await
        .unwrap();
    let snapshot: Snapshot = serde_json::from_value(value).unwrap();
    let mut restored = vt100::Parser::new(snapshot.rows, snapshot.cols, 1000);
    restored.process(&STANDARD.decode(snapshot.data).unwrap());
    assert!(restored.screen().contents().contains("rpc-screen"));
}

#[tokio::test]
async fn u104_output_offsets_are_contiguous_and_snapshot_after_matches_output() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut spawned = terminals
        .spawn(sh(dir.path(), "printf 'alpha'; printf 'beta' "))
        .await
        .unwrap();
    let mut next = 0;
    loop {
        match spawned.events.recv().await.unwrap() {
            EventData::TerminalOutput(out) => {
                assert_eq!(out.offset, next);
                next += STANDARD.decode(out.data).unwrap().len() as u64;
            }
            EventData::TerminalExited(_) => break,
            _ => {}
        }
    }
    assert_eq!(terminals.snapshot(&spawned.id).unwrap().after, next);
}

#[tokio::test]
async fn u104_resize_changes_the_snapshot_size_without_changing_its_output_offset() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut spawned = terminals
        .spawn(sh(dir.path(), "printf ready; read line"))
        .await
        .unwrap();
    until_printed(&mut spawned.events, "ready").await;
    let before = terminals.snapshot(&spawned.id).unwrap();
    terminals.resize(&spawned.id, 100, 30).await.unwrap();
    let after = terminals.snapshot(&spawned.id).unwrap();
    assert_eq!((after.cols, after.rows), (100, 30));
    assert_eq!(after.after, before.after);
    terminals.kill(&spawned.id).await.unwrap();
}

#[tokio::test]
async fn u104_snapshot_keeps_the_newest_thousand_scrollback_lines() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut spawned = terminals
        .spawn(sh(dir.path(), "seq -f 'line-%04g' 1 1100"))
        .await
        .unwrap();
    until_exit(&mut spawned.events).await;
    let snapshot = terminals.snapshot(&spawned.id).unwrap();
    let mut restored = vt100::Parser::new(snapshot.rows, snapshot.cols, 1000);
    restored.process(&STANDARD.decode(snapshot.data).unwrap());
    let mut screen = restored.screen().clone();
    screen.set_scrollback(1000);
    let (_, cols) = screen.size();
    let oldest = screen.rows(0, cols).next().unwrap();
    assert!(oldest.contains("line-0078"), "oldest row: {oldest:?}");
    screen.set_scrollback(0);
    assert!(screen.contents().contains("line-1100"));
}

#[tokio::test]
async fn u104_snapshot_cuts_old_history_before_the_visible_screen_at_one_megabyte() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut params = sh(
        dir.path(),
        r#"awk 'BEGIN { for (i = 1; i <= 1200; i++) { for (j = 1; j <= 995; j++) printf "x"; printf "%04d\n", i } }'"#,
    );
    params.cols = 1000;
    let mut spawned = terminals.spawn(params).await.unwrap();
    until_exit(&mut spawned.events).await;
    let snapshot = terminals.snapshot(&spawned.id).unwrap();
    let data = STANDARD.decode(snapshot.data).unwrap();
    assert!(data.len() <= 1_048_576);
    let mut restored = vt100::Parser::new(snapshot.rows, snapshot.cols, 1000);
    restored.process(&data);
    assert!(restored.screen().contents().contains("1200"));
}

#[tokio::test]
async fn u104_snapshot_screen_and_after_name_the_same_output_cut() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut spawned = terminals
        .spawn(sh(dir.path(), "seq -f 'burst-%04g' 1 300"))
        .await
        .unwrap();
    let mut chunks = Vec::new();
    let snapshot = loop {
        match spawned.events.recv().await.unwrap() {
            EventData::TerminalOutput(out) => {
                chunks.push(out);
                break terminals.snapshot(&spawned.id).unwrap();
            }
            EventData::TerminalExited(_) => panic!("burst produced no output"),
            _ => {}
        }
    };
    loop {
        match spawned.events.recv().await.unwrap() {
            EventData::TerminalOutput(out) => chunks.push(out),
            EventData::TerminalExited(_) => break,
            _ => {}
        }
    }
    let mut before = vt100::Parser::new(snapshot.rows, snapshot.cols, 1000);
    for out in chunks {
        if out.offset >= snapshot.after {
            break;
        }
        let bytes = STANDARD.decode(out.data).unwrap();
        assert!(out.offset + bytes.len() as u64 <= snapshot.after);
        before.process(&bytes);
    }
    let mut restored = vt100::Parser::new(snapshot.rows, snapshot.cols, 1000);
    restored.process(&STANDARD.decode(snapshot.data).unwrap());
    assert_eq!(restored.screen().contents(), before.screen().contents());
}

#[tokio::test]
async fn u104_oversized_window_keeps_titles_and_offsets_but_has_no_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut params = sh(dir.path(), r"printf '\033]0;large\007still-output'");
    (params.cols, params.rows) = (u16::MAX, u16::MAX);
    let mut spawned = terminals.spawn(params).await.unwrap();
    let mut title = None;
    let mut next = 0;
    loop {
        match spawned.events.recv().await.unwrap() {
            EventData::TerminalTitle(event) => title = Some(event.title),
            EventData::TerminalOutput(event) => {
                assert_eq!(event.offset, next);
                next += STANDARD.decode(event.data).unwrap().len() as u64;
            }
            EventData::TerminalExited(_) => break,
            _ => {}
        }
    }
    assert_eq!(title.as_deref(), Some("large"));
    assert!(next > 0);
    assert_eq!(
        terminals.snapshot(&spawned.id).unwrap_err().code,
        code::CONFLICT
    );
}

#[tokio::test]
async fn u104_oversized_visible_screen_is_conflict_instead_of_internal() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut params = sh(
        dir.path(),
        r#"awk 'BEGIN { for (r = 1; r <= 200; r++) for (c = 1; c <= 300; c++) printf "\033[38;2;%d;%d;%dmx", c % 256, r % 256, (r + c) % 256 }'"#,
    );
    (params.cols, params.rows) = (300, 200);
    let mut spawned = terminals.spawn(params).await.unwrap();
    until_exit(&mut spawned.events).await;
    assert_eq!(
        terminals.snapshot(&spawned.id).unwrap_err().code,
        code::CONFLICT
    );
}

#[tokio::test]
async fn u104_shrinking_after_an_oversized_window_cannot_claim_a_complete_screen() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut spawned = terminals
        .spawn(sh(dir.path(), "printf before; read line"))
        .await
        .unwrap();
    until_printed(&mut spawned.events, "before").await;
    terminals
        .resize(&spawned.id, u16::MAX, u16::MAX)
        .await
        .unwrap();
    terminals.resize(&spawned.id, 80, 24).await.unwrap();
    assert_eq!(
        terminals.snapshot(&spawned.id).unwrap_err().code,
        code::CONFLICT
    );
    terminals.kill(&spawned.id).await.unwrap();
}

#[tokio::test]
async fn u104_resize_that_would_grow_history_past_its_budget_is_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut spawned = terminals
        .spawn(sh(dir.path(), "printf before; read line"))
        .await
        .unwrap();
    until_printed(&mut spawned.events, "before").await;
    terminals.resize(&spawned.id, 1000, 40).await.unwrap();
    assert_eq!(
        terminals.snapshot(&spawned.id).unwrap_err().code,
        code::CONFLICT
    );
    terminals.kill(&spawned.id).await.unwrap();
}
