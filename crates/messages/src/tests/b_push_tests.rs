//! B22, B23: the caps on pushed digests and what survives a restart. `top` is a Door that is
//! `working`, so every push to it waits as `pending`. The clock is fake.

use std::sync::atomic::{AtomicI64, Ordering};

use contracts::agent::DigestEntry;
use serde_json::{Value, json};

use super::b_hop_tests::tree;
use super::*;
use super::{FakeRail, agent};

struct Pushes {
    messages: Arc<Messages>,
    bus: Bus,
    rail: Arc<FakeRail>,
    clock: Arc<AtomicI64>,
    touches: Arc<provenance::Touches>,
    dir: std::path::PathBuf,
}

fn entry(name: &str, todos: u32) -> DigestEntry {
    DigestEntry {
        name: name.into(),
        kind: Kind::Working,
        last: "x".into(),
        todos,
        pads: vec![],
    }
}

impl Pushes {
    fn new(dir: &Path) -> Self {
        Self::over(dir, FakeRail::new(tree()), Arc::default())
    }

    fn over(dir: &Path, rail: Arc<FakeRail>, clock: Arc<AtomicI64>) -> Self {
        let bus = Bus::new();
        let touches = Arc::new(provenance::Touches::in_memory().unwrap());
        let reads = Arc::clone(&clock);
        let env = Env {
            touches: Arc::clone(&touches),
            clock: Arc::new(move || reads.load(Ordering::SeqCst)),
            fault: Arc::new(|_| Ok(())),
        };
        let agents: Arc<dyn Module> = rail.clone();
        let messages =
            Arc::new(Messages::open_with(dir, bus.clone(), agents, noop_deliver(), env).unwrap());
        Self {
            messages,
            bus,
            rail,
            clock,
            touches,
            dir: dir.to_path_buf(),
        }
    }

    fn restarted(self) -> Self {
        let Self {
            rail, clock, dir, ..
        } = self;
        Self::over(&dir, rail, clock)
    }

    async fn call_as(&self, actor: Actor, method: &str, p: Value) -> Result<Value, RpcError> {
        let ctx = Ctx {
            actor,
            bus: self.bus.clone(),
            touches: Arc::clone(&self.touches),
        };
        self.messages.call(&ctx, method, p).await
    }

    async fn push(&self, entry: &DigestEntry) {
        self.messages.push_digest("top", entry).await.unwrap();
    }

    /// The Daemon's clock reads `ms`, and the window's end is acted on.
    async fn at(&self, ms: i64) {
        self.clock.store(ms, Ordering::SeqCst);
        pushes::flush(&self.messages.inner).await;
    }

    async fn all(&self) -> Vec<Message> {
        let all = self
            .call_as(Actor::user(), "message.list", json!({}))
            .await
            .unwrap();
        serde_json::from_value(all).unwrap()
    }

    /// The bodies of the pushed digests to `top` that still stand: not `dropped`.
    async fn standing(&self) -> Vec<Message> {
        self.all()
            .await
            .into_iter()
            .filter(|m| pushes::is_digest(&m.from, &m.body) && m.status != MessageStatus::Dropped)
            .collect()
    }
}

#[tokio::test]
async fn b22_the_ninth_push_in_a_window_waits_and_leaves_at_its_end_as_one_rollup() {
    let dir = tempfile::tempdir().unwrap();
    let h = Pushes::new(dir.path());
    for n in 0..11 {
        h.at(n).await;
        h.push(&entry(&format!("c{}", n % 2), n as u32)).await;
    }
    assert_eq!(h.standing().await.len(), 8);

    h.at(59_999).await;
    assert_eq!(
        h.standing().await.len(),
        8,
        "the first push is 59 999 ms old"
    );
    h.at(60_000).await;
    let standing = h.standing().await;
    assert_eq!(standing.len(), 9);
    let rollup = &standing[8];
    assert!(
        rollup.body.starts_with("[digest] [3 updates held back]\n"),
        "{}",
        rollup.body
    );
    // The newest change of each child first: c0 changed at 10, c1 at 9.
    let lines: Vec<DigestEntry> = rollup
        .body
        .lines()
        .skip(1)
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        lines
            .iter()
            .map(|e| (e.name.as_str(), e.todos))
            .collect::<Vec<_>>(),
        [("c0", 10), ("c1", 9)]
    );
    assert_eq!(rollup.from, Actor::daemon());
    assert_eq!(rollup.kind, MessageKind::Note);
}

#[tokio::test]
async fn b22_a_rollup_names_eight_children_and_the_rest_in_one_line_cut_at_512_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let h = Pushes::new(dir.path());
    for n in 0..8 {
        h.push(&entry("early", n)).await;
    }
    for n in 0..30 {
        h.push(&entry(&format!("child-{n}-{}", "w".repeat(30)), 0))
            .await;
    }
    h.at(60_000).await;
    let rollup = h.standing().await.pop().unwrap().body;
    assert!(rollup.starts_with("[digest] [30 updates held back]\n"));
    assert!(rollup.len() <= 4096, "{}", rollup.len());
    let lines: Vec<&str> = rollup.lines().collect();
    assert_eq!(lines.len(), 1 + 8 + 1);
    // Newest change first: the last child pushed leads.
    assert!(lines[1].contains("child-29-"));
    let more = lines[9];
    assert!(more.starts_with("[+22 more: child-21-"), "{more}");
    assert!(more.len() <= 512 && more.ends_with("[cut]"), "{more}");
}

#[tokio::test]
async fn b22_the_twenty_first_open_digest_merges_the_oldest_two() {
    let dir = tempfile::tempdir().unwrap();
    let h = Pushes::new(dir.path());
    for n in 0..21_u32 {
        h.at(i64::from(n / 8) * 60_000).await;
        h.push(&entry(&format!("c{}", n % 2), n)).await;
    }
    let all = h.all().await;
    let merged: Vec<_> = all
        .iter()
        .filter(|m| m.status == MessageStatus::Dropped)
        .collect();
    assert_eq!(merged.iter().map(|m| m.id).collect::<Vec<_>>(), [1, 2]);
    assert!(merged.iter().all(|m| m.reason == Some(Reason::Merged)));
    let standing = h.standing().await;
    assert_eq!(standing.len(), 20);
    let last = standing.last().unwrap();
    assert!(
        last.body
            .starts_with("[digest] [2 earlier updates: #1 #2]\n"),
        "{}",
        last.body
    );
    // The newest entry of each child the two named: #2 changed c1, #1 changed c0.
    let names: Vec<String> = last
        .body
        .lines()
        .skip(1)
        .map(|line| serde_json::from_str::<DigestEntry>(line).unwrap().name)
        .collect();
    assert_eq!(names, ["c1", "c0"]);
    // `m` reads a Message that was merged away (P7).
    let got = h
        .call_as(agent("top"), "message.get", json!({"id": 1}))
        .await
        .unwrap();
    assert_eq!(got["reason"], "merged");
    assert_eq!(pushes::weight(&last.body), 2);
}

#[tokio::test]
async fn b22_a_merge_counts_the_changes_a_merged_digest_already_held() {
    let dir = tempfile::tempdir().unwrap();
    let h = Pushes::new(dir.path());
    for n in 0..60_u32 {
        h.at(i64::from(n / 8) * 60_000).await;
        h.push(&entry("c", n)).await;
    }
    let standing = h.standing().await;
    assert_eq!(standing.len(), 20);
    assert_eq!(
        standing
            .iter()
            .map(|m| pushes::weight(&m.body))
            .sum::<u32>(),
        60
    );
    // A merge that is itself merged later weighs its own N.
    assert!(standing.iter().any(|m| pushes::weight(&m.body) >= 3));
}

#[tokio::test]
async fn b22_after_a_burst_of_100_changes_with_a_takeover_every_change_is_counted_once() {
    let dir = tempfile::tempdir().unwrap();
    let h = Pushes::new(dir.path());
    let mut changes = 0_u32;
    for round in 0..10_i64 {
        if round == 3 {
            h.call_as(Actor::user(), "takeover.begin", json!({"agent": "top"}))
                .await
                .unwrap();
        }
        if round == 7 {
            let held = h.standing().await;
            assert!(held.iter().any(|m| m.reason == Some(Reason::Takeover)));
            h.call_as(Actor::user(), "takeover.end", json!({"agent": "top"}))
                .await
                .unwrap();
        }
        h.at(round * 20_000).await;
        for n in 0..10_u32 {
            h.push(&entry(&format!("c{}", n % 4), changes)).await;
            changes += 1;
        }
    }
    h.at(10 * 60_000 + 1_000_000).await;
    h.at(2 * (10 * 60_000 + 1_000_000)).await;

    let all = h.all().await;
    let digests: Vec<&Message> = all
        .iter()
        .filter(|m| pushes::is_digest(&m.from, &m.body))
        .collect();
    let standing: Vec<&&Message> = digests
        .iter()
        .filter(|m| m.status != MessageStatus::Dropped)
        .collect();
    assert_eq!(changes, 100);
    assert_eq!(
        standing
            .iter()
            .map(|m| pushes::weight(&m.body))
            .sum::<u32>(),
        100
    );
    assert!(standing.len() <= 20);
    // Every digest is standing, or named by a merge, and `m` can open each.
    for dropped in digests
        .iter()
        .filter(|m| m.status == MessageStatus::Dropped)
    {
        assert_eq!(dropped.reason, Some(Reason::Merged));
        let tag = format!("#{}", dropped.id);
        assert!(
            digests
                .iter()
                .any(|m| m.body.lines().next().is_some_and(|l| l.contains(&tag))),
            "{} is named by no merge",
            dropped.id
        );
    }
    for message in &digests {
        h.call_as(agent("top"), "message.get", json!({"id": message.id}))
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn b22_pushes_never_get_conflict_and_do_not_fill_the_bound_of_32() {
    let dir = tempfile::tempdir().unwrap();
    let h = Pushes::new(dir.path());
    h.call_as(Actor::user(), "takeover.begin", json!({"agent": "top"}))
        .await
        .unwrap();
    for _ in 0..32 {
        h.call_as(
            agent("c"),
            "message.send",
            json!({"to": "top", "kind": "note", "body": "hi"}),
        )
        .await
        .unwrap();
    }
    let refused = h
        .call_as(
            agent("c"),
            "message.send",
            json!({"to": "top", "kind": "note", "body": "hi"}),
        )
        .await;
    assert_eq!(refused.unwrap_err().code, code::CONFLICT);

    for n in 0..8 {
        h.push(&entry("c", n)).await;
    }
    assert_eq!(h.standing().await.len(), 8);
    assert!(
        h.standing()
            .await
            .iter()
            .all(|m| m.status == MessageStatus::Held)
    );
    // Twenty open digests do not make room for nor take from the Agents' 32.
    let again = h
        .call_as(
            agent("d"),
            "message.send",
            json!({"to": "top", "kind": "note", "body": "hi"}),
        )
        .await;
    assert_eq!(again.unwrap_err().code, code::CONFLICT);
}

#[tokio::test]
async fn b23_pending_pushes_and_a_rollup_survive_a_restart_and_the_window_starts_again() {
    let dir = tempfile::tempdir().unwrap();
    let h = Pushes::new(dir.path());
    for n in 0..11 {
        h.push(&entry("c", n)).await;
    }
    h.at(60_000).await;
    let before = h.standing().await;
    assert_eq!(before.len(), 9);

    let h = h.restarted();
    let after = h.standing().await;
    assert_eq!(
        after
            .iter()
            .map(|m| (m.id, m.body.clone(), m.status))
            .collect::<Vec<_>>(),
        before
            .iter()
            .map(|m| (m.id, m.body.clone(), m.status))
            .collect::<Vec<_>>()
    );
    assert_eq!(pushes::weight(&after[8].body), 3);

    // The same instant: the window is empty again, so eight pushes go out at once.
    for n in 0..8 {
        h.push(&entry("c", n)).await;
    }
    assert_eq!(
        h.all()
            .await
            .iter()
            .filter(|m| m.body.starts_with("[digest] {"))
            .count(),
        8 + 8
    );
}
