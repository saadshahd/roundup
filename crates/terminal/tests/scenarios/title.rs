//! X4 title.

use crate::common::{open, sh};
use contracts::EventData;
use contracts::terminal::SpawnParams;

async fn titles_until_exit(script: &str) -> (Vec<String>, Vec<Option<String>>) {
    let dir = tempfile::tempdir().unwrap();
    let params = sh(dir.path(), script);
    titles_of(dir, params).await
}

async fn titles_of(
    dir: tempfile::TempDir,
    params: SpawnParams,
) -> (Vec<String>, Vec<Option<String>>) {
    let (terminals, _) = open(&dir);
    let mut spawned = terminals.spawn(params).await.unwrap();
    let mut titles = Vec::new();
    loop {
        match spawned.events.recv().await.unwrap() {
            EventData::TerminalTitle(title) => titles.push(title.title),
            EventData::TerminalExited(_) => break,
            _ => {}
        }
    }
    let listed = terminals.list().into_iter().map(|t| t.title).collect();
    (titles, listed)
}

#[tokio::test]
async fn x4_title_is_emitted_and_listed() {
    let (titles, listed) = titles_until_exit(r"printf '\033]0;hello\007'").await;
    assert_eq!(titles, ["hello"]);
    assert_eq!(listed, [Some("hello".to_string())]);
}

#[tokio::test]
async fn x4_a_later_title_replaces_it() {
    let (titles, listed) = titles_until_exit(r"printf '\033]0;one\007\033]2;two\033\\'").await;
    assert_eq!(titles, ["one", "two"]);
    assert_eq!(listed, [Some("two".to_string())]);
}

#[tokio::test]
async fn x4_a_title_split_across_reads_is_whole() {
    let (titles, _) = titles_until_exit(r"printf '\033]0;hel'; sleep 0.3; printf 'lo\007'").await;
    assert_eq!(titles, ["hello"]);
}

#[tokio::test]
async fn x4_titles_do_not_depend_on_the_window_size() {
    let dir = tempfile::tempdir().unwrap();
    let mut params = sh(dir.path(), r"printf '\033]0;big\007'");
    (params.cols, params.rows) = (u16::MAX, u16::MAX);
    let (titles, _) = titles_of(dir, params).await;
    assert_eq!(titles, ["big"]);
}
