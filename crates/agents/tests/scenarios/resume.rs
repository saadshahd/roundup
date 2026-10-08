//! A21 saving an Agent's conversation, and A22 resuming it.

use std::sync::Arc;
use std::time::Duration;

use contracts::Kind;
use contracts::agent::RailNode;
use rpc::code;
use serde_json::{Value, json};

use crate::common::{Fixture, status_of};

const CONVERSATION: &str = "11111111-1111-1111-1111-111111111111";
const OTHER_CONVERSATION: &str = "22222222-2222-2222-2222-222222222222";

impl Fixture {
    async fn signal_payload(
        &self,
        id: &str,
        attempt: &str,
        payload: Value,
    ) -> Result<Value, rpc::RpcError> {
        self.call(
            "agent.signal",
            json!({"id": id, "attempt": attempt, "payload": payload}),
        )
        .await
    }

    /// A `SessionStart` of `id`'s current Attempt, naming `conversation` with `source`.
    async fn session_start(
        &self,
        id: &str,
        conversation: &str,
        source: &str,
    ) -> Result<Value, rpc::RpcError> {
        let attempt = self.attempt(id).await;
        self.signal_payload(
            id,
            &attempt,
            json!({"hook_event_name": "SessionStart", "session_id": conversation, "source": source}),
        )
        .await
    }

    async fn resume(&self, id: &str) -> Result<RailNode, rpc::RpcError> {
        let node = self.call("agent.resume", json!({"id": id})).await?;
        Ok(serde_json::from_value(node).unwrap())
    }

    fn can_resume(&self, tree: &[RailNode], id: &str) -> bool {
        tree.iter().find(|n| n.id == id).expect("node").can_resume
    }

    /// The tree once `id`'s Attempt differs from `old`, for a caller racing a pending
    /// `agent.resume` (which has already allocated and announced the new one before it blocks
    /// on acknowledgement).
    async fn until_attempt_changed(&self, id: &str, old: &str) -> Vec<RailNode> {
        self.until(|tree| {
            tree.iter()
                .find(|n| n.id == id)
                .and_then(|n| n.attempt.as_deref())
                != Some(old)
        })
        .await
    }
}

/// Hold a second write transaction open on `agents.db`, so a write through the Fixture's own
/// connection fails with a real SQLite error once rusqlite's default busy_timeout elapses
/// (mirrors `crates/agents/src/lib.rs`'s own `lock_db_for_writes`, unavailable here since `Rail`
/// is private to the `agents` crate).
fn lock_db_for_writes(f: &Fixture) -> rusqlite::Connection {
    let lock = rusqlite::Connection::open(f.dir.path().join("agents.db")).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE;").unwrap();
    lock
}

async fn exited_with_conversation(f: &Fixture, conversation: &str) -> RailNode {
    let node = f.spawn(None, None).await.unwrap();
    f.session_start(&node.id, conversation, "startup")
        .await
        .unwrap();
    f.call("agent.stop", json!({"id": node.id})).await.unwrap();
    f.tree()
        .await
        .into_iter()
        .find(|n| n.id == node.id)
        .unwrap()
}

#[tokio::test]
async fn a21_a_session_start_saves_the_conversation_so_an_exited_agent_can_resume() {
    let f = Fixture::running("sleep 30");
    let node = exited_with_conversation(&f, CONVERSATION).await;

    assert!(node.can_resume);
}

#[tokio::test]
async fn a21_a_running_agent_cannot_resume_even_with_saved_data() {
    let f = Fixture::running("sleep 30");
    let node = f.spawn(None, None).await.unwrap();

    f.session_start(&node.id, CONVERSATION, "startup")
        .await
        .unwrap();

    let tree = f.tree().await;
    assert!(!f.can_resume(&tree, &node.id));
}

#[tokio::test]
async fn a21_a_non_uuid_session_id_is_not_saved() {
    let f = Fixture::running("sleep 30");
    let node = f.spawn(None, None).await.unwrap();

    f.session_start(&node.id, "not-a-uuid", "startup")
        .await
        .unwrap();
    f.call("agent.stop", json!({"id": node.id})).await.unwrap();

    let tree = f.tree().await;
    assert!(!f.can_resume(&tree, &node.id));
}

#[tokio::test]
async fn a21_a_later_session_start_overrides_the_earlier_saved_conversation() {
    let f = Fixture::running("sleep 30");
    let node = f.spawn(None, None).await.unwrap();
    f.session_start(&node.id, CONVERSATION, "startup")
        .await
        .unwrap();

    f.session_start(&node.id, OTHER_CONVERSATION, "startup")
        .await
        .unwrap();
    f.call("agent.stop", json!({"id": node.id})).await.unwrap();

    // The resume below only acknowledges with the later id; a mismatched ack (the first,
    // overridden one) would time out instead of promoting, which `a22_...timeout` proves
    // separately, so succeeding here is itself the proof the later save won.
    let f = Arc::new(f);
    let old_attempt = f.attempt(&node.id).await;
    let id = node.id.clone();
    let resuming = {
        let f = Arc::clone(&f);
        let id = id.clone();
        tokio::spawn(async move { f.resume(&id).await })
    };
    f.until_attempt_changed(&id, &old_attempt).await;
    f.session_start(&id, OTHER_CONVERSATION, "resume")
        .await
        .unwrap();
    let resumed = resuming.await.unwrap().unwrap();

    assert!(resumed.terminal_id.is_some());
}

#[tokio::test]
async fn a21_a_failed_save_after_spawn_returned_errors_settles_error_and_stops_the_terminal() {
    let f = Fixture::running("sleep 30");
    let node = f.spawn(None, None).await.unwrap();
    let lock = lock_db_for_writes(&f);

    let err = f.session_start(&node.id, CONVERSATION, "startup").await;

    drop(lock);
    assert!(err.is_err());
    let tree = f
        .until(|tree| status_of(tree, &node.id).kind == Kind::Error)
        .await;
    let status = status_of(&tree, &node.id);
    assert!(status.label.contains("cannot save conversation"));
    assert!(!f.can_resume(&tree, &node.id));
}

#[tokio::test]
async fn a21_a_failed_save_while_held_fails_the_whole_spawn() {
    let f = Arc::new(Fixture::running("sleep 30"));
    let config = crate::common::hold_starts(&f);
    let spawning = {
        let f = Arc::clone(&f);
        let cwd = f.dir.path().to_string_lossy().into_owned();
        tokio::spawn(async move {
            f.call(
                "agent.spawn",
                json!({"cwd": cwd, "prompt": null, "parent": null}),
            )
            .await
        })
    };
    // The node exists (and is `Starting`) as soon as `agent.spawn` reserves it, well before
    // `start` reaches the held config read; only then is the DB write locked, so the node's own
    // creation is never what the lock blocks.
    let id = f
        .until(|tree| !tree.is_empty())
        .await
        .first()
        .map(|n| n.id.clone())
        .expect("the node exists while its launch is held");
    let lock = lock_db_for_writes(&f);
    // The held SessionStart reaches `agent.signal` before the config read `start` is waiting on
    // returns, exactly as A14's own window does.
    f.signal_payload(
        &id,
        "1",
        json!({"hook_event_name": "SessionStart", "session_id": CONVERSATION, "source": "startup"}),
    )
    .await
    .unwrap();
    crate::common::release(config).await;

    let result = spawning.await.unwrap();

    drop(lock);
    assert!(
        result.is_err(),
        "a held conversation save failure must fail the spawn"
    );
    // The orphaned node's own removal needs the same DB this test just locked, so it is proven
    // at the unit level (`a21_a_failed_held_save_reinserts_so_the_caller_can_clean_up`,
    // `crates/agents/src/lib.rs`), which controls the lock's release precisely; here, once the
    // lock is gone, a retry of what `agent.spawn`'s own cleanup already tried succeeds.
    assert_eq!(
        f.tree().await.len(),
        1,
        "exactly the orphaned node remains, locked out of its own cleanup"
    );
}

#[tokio::test]
async fn a22_resume_launches_with_resume_flag_and_promotes_on_ack() {
    let f = Fixture::running("echo \"$@\" > argv.txt; sleep 30");
    let node = exited_with_conversation(&f, CONVERSATION).await;
    let f = Arc::new(f);
    let old_attempt = node.attempt.clone().unwrap();

    let resuming = {
        let f = Arc::clone(&f);
        let id = node.id.clone();
        tokio::spawn(async move { f.resume(&id).await })
    };
    let pending = f.until_attempt_changed(&node.id, &old_attempt).await;
    assert!(
        !f.can_resume(&pending, &node.id),
        "pending while unacknowledged"
    );
    f.session_start(&node.id, CONVERSATION, "resume")
        .await
        .unwrap();

    let resumed = resuming.await.unwrap().unwrap();

    assert_ne!(resumed.attempt.as_deref(), Some(old_attempt.as_str()));
    assert!(resumed.terminal_id.is_some());
    assert_eq!(resumed.status.as_ref().map(|s| s.kind), Some(Kind::Idle));
    assert_eq!(resumed.id, node.id);
    assert_eq!(resumed.name, node.name);
    assert_eq!(resumed.parent, node.parent);
    let argv = crate::common::until_file(&f.dir.path().join("argv.txt")).await;
    assert!(argv.contains(&format!("--resume {CONVERSATION}")));
}

#[tokio::test]
async fn a22_an_id_that_is_not_an_agent_is_conflict() {
    let f = Fixture::running("sleep 30");
    let room = f.room("r", None).await;

    let err = f.resume(&room).await.unwrap_err();

    assert_eq!(err.code, code::CONFLICT);
}

#[tokio::test]
async fn a22_an_unknown_id_is_not_found() {
    let f = Fixture::running("sleep 30");

    let err = f.resume("999").await.unwrap_err();

    assert_eq!(err.code, code::NOT_FOUND);
}

#[tokio::test]
async fn a22_a_running_agent_cannot_be_resumed() {
    let f = Fixture::running("sleep 30");
    let node = f.spawn(None, None).await.unwrap();

    let err = f.resume(&node.id).await.unwrap_err();

    assert_eq!(err.code, code::CONFLICT);
}

#[tokio::test]
async fn a22_an_exited_agent_with_no_saved_conversation_cannot_be_resumed() {
    let f = Fixture::running("sleep 30");
    let node = f.spawn(None, None).await.unwrap();
    f.call("agent.stop", json!({"id": node.id})).await.unwrap();

    let err = f.resume(&node.id).await.unwrap_err();

    assert_eq!(err.code, code::CONFLICT);
}

#[tokio::test]
async fn a22_a_concurrent_resume_is_conflict() {
    let f = Fixture::running("sleep 30");
    let node = exited_with_conversation(&f, CONVERSATION).await;
    let f = Arc::new(f);
    let old_attempt = node.attempt.clone().unwrap();
    let id = node.id.clone();
    let first = {
        let f = Arc::clone(&f);
        let id = id.clone();
        tokio::spawn(async move { f.resume(&id).await })
    };
    f.until_attempt_changed(&id, &old_attempt).await;

    let second = f.resume(&id).await;

    assert_eq!(second.unwrap_err().code, code::CONFLICT);
    f.session_start(&id, CONVERSATION, "resume").await.unwrap();
    first.await.unwrap().unwrap();
}

#[tokio::test]
async fn a22_stop_during_a_pending_resume_is_conflict_and_resume_still_resolves() {
    let f = Fixture::running("sleep 30");
    let node = exited_with_conversation(&f, CONVERSATION).await;
    let f = Arc::new(f);
    let old_attempt = node.attempt.clone().unwrap();
    let id = node.id.clone();
    let resuming = {
        let f = Arc::clone(&f);
        let id = id.clone();
        tokio::spawn(async move { f.resume(&id).await })
    };
    f.until_attempt_changed(&id, &old_attempt).await;

    let stop = f.call("agent.stop", json!({"id": id})).await;

    assert_eq!(stop.unwrap_err().code, code::CONFLICT);
    f.session_start(&id, CONVERSATION, "resume").await.unwrap();
    resuming.await.unwrap().unwrap();
}

#[tokio::test]
async fn a22_a_failed_launch_rolls_back_to_the_old_node() {
    let f = Fixture::running("sleep 30");
    let node = exited_with_conversation(&f, CONVERSATION).await;
    let before = f
        .tree()
        .await
        .into_iter()
        .find(|n| n.id == node.id)
        .unwrap();
    // `claude` itself cannot run at all, so the Terminal spawn fails, same seam
    // `a4_a_missing_claude_leaves_no_agent_and_no_settings_file` uses.
    std::fs::remove_file(f.dir.path().join("fake-claude")).unwrap();

    let err = f.resume(&node.id).await.unwrap_err();

    let after = f
        .tree()
        .await
        .into_iter()
        .find(|n| n.id == node.id)
        .unwrap();
    assert_ne!(err.code, code::NOT_FOUND);
    assert!(after.can_resume, "the old node must stay resumable");
    assert_eq!(after.terminal_id, before.terminal_id);
    assert_eq!(after.status, before.status);
}

#[tokio::test]
async fn a22_can_resume_survives_a_reopen() {
    let f = Fixture::running("sleep 30");
    let node = exited_with_conversation(&f, CONVERSATION).await;
    assert!(
        f.tree()
            .await
            .into_iter()
            .find(|n| n.id == node.id)
            .unwrap()
            .can_resume
    );

    let f = f.reopen();

    let tree = f.tree().await;
    let reopened = tree.iter().find(|n| n.id == node.id).unwrap();
    assert!(reopened.can_resume);
    assert_eq!(reopened.terminal_id, None);
}

#[tokio::test(start_paused = true)]
async fn a22_no_acknowledgement_within_ten_seconds_times_out_and_rolls_back() {
    let f = Fixture::running("sleep 30");
    let node = exited_with_conversation(&f, CONVERSATION).await;
    let f = Arc::new(f);
    let old_attempt = node.attempt.clone().unwrap();
    let id = node.id.clone();

    let resuming = {
        let f = Arc::clone(&f);
        let id = id.clone();
        tokio::spawn(async move { f.resume(&id).await })
    };
    f.until_attempt_changed(&id, &old_attempt).await;
    tokio::time::advance(Duration::from_secs(11)).await;

    let err = resuming.await.unwrap().unwrap_err();

    assert_eq!(err.code, code::CONFLICT);
    let tree = f.tree().await;
    let after = tree.iter().find(|n| n.id == id).unwrap();
    assert!(after.can_resume);
    assert_eq!(after.terminal_id, None);
}

/// A `SessionStart` is the ack only with source `resume` naming the saved id; anything else is
/// held exactly as a non-`SessionStart` Signal is, never promoting the pending resume. Proven by
/// timing it out (success would mean the mismatched ack was mistaken for the real one) rather
/// than by only sending the real one afterward, which would pass even if the check were a no-op.
async fn held_as_not_the_ack(session_id: &str, source: &str) {
    let f = Fixture::running("sleep 30");
    let node = exited_with_conversation(&f, CONVERSATION).await;
    let f = Arc::new(f);
    let old_attempt = node.attempt.clone().unwrap();
    let id = node.id.clone();
    let resuming = {
        let f = Arc::clone(&f);
        let id = id.clone();
        tokio::spawn(async move { f.resume(&id).await })
    };
    f.until_attempt_changed(&id, &old_attempt).await;

    f.session_start(&id, session_id, source).await.unwrap();
    tokio::time::advance(Duration::from_secs(11)).await;

    let err = resuming.await.unwrap().unwrap_err();
    assert_eq!(err.code, code::CONFLICT);
}

#[tokio::test(start_paused = true)]
async fn a22_an_ack_with_the_wrong_source_is_held_not_promoted() {
    held_as_not_the_ack(CONVERSATION, "startup").await;
}

#[tokio::test(start_paused = true)]
async fn a22_an_ack_naming_a_different_conversation_is_held_not_promoted() {
    held_as_not_the_ack(OTHER_CONVERSATION, "resume").await;
}

/// A non-ack held first, then the real ack: proves a mismatch is held (not rejected outright)
/// and the resume still resolves once the real one arrives.
#[tokio::test]
async fn a22_a_mismatched_signal_held_before_the_real_ack_still_lets_it_resolve() {
    let f = Fixture::running("sleep 30");
    let node = exited_with_conversation(&f, CONVERSATION).await;
    let f = Arc::new(f);
    let old_attempt = node.attempt.clone().unwrap();
    let id = node.id.clone();
    let resuming = {
        let f = Arc::clone(&f);
        let id = id.clone();
        tokio::spawn(async move { f.resume(&id).await })
    };
    f.until_attempt_changed(&id, &old_attempt).await;

    f.session_start(&id, OTHER_CONVERSATION, "resume")
        .await
        .unwrap();
    f.session_start(&id, CONVERSATION, "resume").await.unwrap();

    let resumed = resuming.await.unwrap().unwrap();
    assert!(resumed.terminal_id.is_some());
}

#[tokio::test]
async fn a22_resume_in_a_worktree_subfolder_keeps_the_saved_subfolder_not_the_worktree_root() {
    let f = Fixture::in_git_project("sleep 30", agents::worktree::Git::from_env());
    f.call(
        "project.setWorktrees",
        json!({"on": true, "check": Value::Null}),
    )
    .await
    .unwrap();
    let sub = f.dir.path().join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("keep.txt"), "x").unwrap();
    let git = |args: &[&str]| {
        assert!(
            std::process::Command::new("git")
                .current_dir(f.dir.path())
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    };
    git(&["add", "sub/keep.txt"]);
    git(&["commit", "-q", "-m", "add sub"]);

    let spawned = f
        .call(
            "agent.spawn",
            json!({"cwd": sub.to_string_lossy(), "prompt": Value::Null, "parent": Value::Null}),
        )
        .await
        .unwrap();
    let node: RailNode = serde_json::from_value(spawned).unwrap();
    let worktree_path = std::path::PathBuf::from(&node.worktree.as_ref().unwrap().path);
    f.session_start(&node.id, CONVERSATION, "startup")
        .await
        .unwrap();
    f.call("agent.stop", json!({"id": node.id})).await.unwrap();
    let f = Arc::new(f);
    let old_attempt = node.attempt.clone().unwrap();
    let id = node.id.clone();
    let resuming = {
        let f = Arc::clone(&f);
        let id = id.clone();
        tokio::spawn(async move { f.resume(&id).await })
    };
    f.until_attempt_changed(&id, &old_attempt).await;
    f.session_start(&id, CONVERSATION, "resume").await.unwrap();

    let resumed = resuming.await.unwrap().unwrap();

    let terminal_id = resumed.terminal_id.clone().unwrap();
    let cwd = f
        .terminals
        .list()
        .into_iter()
        .find(|t| t.id == terminal_id)
        .unwrap()
        .cwd;
    assert_eq!(cwd, worktree_path.join("sub").to_string_lossy());
}

/// A23: a Room whose Door started, saved `CONVERSATION` and was stopped.
async fn exited_door(f: &Fixture) -> RailNode {
    let room = f.room("team", None).await;
    f.call("rail.startDoor", json!({"id": room})).await.unwrap();
    f.session_start(&room, CONVERSATION, "startup")
        .await
        .unwrap();
    f.call("agent.stop", json!({"id": room})).await.unwrap();
    f.tree().await.into_iter().find(|n| n.id == room).unwrap()
}

/// The fake `claude`'s recorded argv once a launch with `--resume` has written its line.
async fn until_resumed(f: &Fixture) -> String {
    for _ in 0..500 {
        let text = std::fs::read_to_string(f.dir.path().join("argv.txt")).unwrap_or_default();
        if text.contains("--resume") {
            return text;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("no launch ever resumed");
}

async fn start_door(f: &Fixture, id: &str) -> Result<RailNode, rpc::RpcError> {
    let node = f.call("rail.startDoor", json!({"id": id})).await?;
    Ok(serde_json::from_value(node).unwrap())
}

#[tokio::test]
async fn a23_a_restarted_door_resumes_its_saved_conversation() {
    let f = Fixture::running("echo \"$@\" >> \"$(dirname \"$0\")/argv.txt\"; sleep 30");
    let door = exited_door(&f).await;
    assert!(door.can_resume);
    let f = Arc::new(f);
    let old_attempt = door.attempt.clone().unwrap();

    let starting = {
        let (f, id) = (Arc::clone(&f), door.id.clone());
        tokio::spawn(async move { start_door(&f, &id).await })
    };
    f.until_attempt_changed(&door.id, &old_attempt).await;
    f.session_start(&door.id, CONVERSATION, "resume")
        .await
        .unwrap();
    let restarted = starting.await.unwrap().unwrap();

    assert_eq!(restarted.id, door.id);
    assert_eq!(restarted.kind, door.kind);
    assert!(restarted.terminal_id.is_some());
    assert_ne!(restarted.attempt, door.attempt);
    let argv = until_resumed(&f).await;
    assert_eq!(
        argv.matches("--resume").count(),
        1,
        "only the restart resumes"
    );
    assert!(argv.contains(&format!("--resume {CONVERSATION}")));
}

#[tokio::test]
async fn a23_a_door_with_no_saved_conversation_starts_fresh() {
    let f = Fixture::running("echo \"$@\" >> \"$(dirname \"$0\")/argv.txt\"; sleep 30");
    let room = f.room("team", None).await;

    let door = start_door(&f, &room).await.unwrap();

    assert!(door.terminal_id.is_some());
    let argv = crate::common::until_file(&f.dir.path().join("argv.txt")).await;
    assert!(!argv.contains("--resume"));
}

#[tokio::test]
async fn a23_a_restarted_door_after_reopen_resumes() {
    let f = Fixture::running("echo \"$@\" >> \"$(dirname \"$0\")/argv.txt\"; sleep 30");
    let door = exited_door(&f).await;
    let f = Arc::new(f.reopen());
    let reopened = f
        .tree()
        .await
        .into_iter()
        .find(|n| n.id == door.id)
        .unwrap();
    assert!(reopened.can_resume);
    let old_attempt = reopened.attempt.clone().unwrap();

    let starting = {
        let (f, id) = (Arc::clone(&f), door.id.clone());
        tokio::spawn(async move { start_door(&f, &id).await })
    };
    f.until_attempt_changed(&door.id, &old_attempt).await;
    f.session_start(&door.id, CONVERSATION, "resume")
        .await
        .unwrap();

    assert!(starting.await.unwrap().unwrap().terminal_id.is_some());
    let argv = crate::common::until_file(&f.dir.path().join("argv.txt")).await;
    assert!(argv.contains(&format!("--resume {CONVERSATION}")));
}

#[tokio::test]
async fn a23_a_saved_conversation_that_cannot_resume_never_starts_fresh() {
    let f = Fixture::running("echo \"$@\" >> \"$(dirname \"$0\")/argv.txt\"; sleep 30");
    let door = exited_door(&f).await;
    let launches = std::fs::read_to_string(f.dir.path().join("argv.txt"))
        .unwrap_or_default()
        .lines()
        .count();
    std::fs::remove_file(f.dir.path().join("fake-claude")).unwrap();

    let err = start_door(&f, &door.id).await.unwrap_err();

    assert_ne!(err.code, code::NOT_FOUND);
    let after = f
        .tree()
        .await
        .into_iter()
        .find(|n| n.id == door.id)
        .unwrap();
    assert!(after.can_resume, "still resumable, not replaced");
    assert_eq!(after.terminal_id, door.terminal_id);
    let now = std::fs::read_to_string(f.dir.path().join("argv.txt"))
        .unwrap_or_default()
        .lines()
        .count();
    assert_eq!(now, launches, "no fresh program ran");
}

#[tokio::test(start_paused = true)]
async fn a23_a_restart_without_acknowledgement_fails_with_a22s_error() {
    let f = Fixture::running("sleep 30");
    let door = exited_door(&f).await;
    let f = Arc::new(f);
    let old_attempt = door.attempt.clone().unwrap();
    let starting = {
        let (f, id) = (Arc::clone(&f), door.id.clone());
        tokio::spawn(async move { start_door(&f, &id).await })
    };
    f.until_attempt_changed(&door.id, &old_attempt).await;
    tokio::time::advance(Duration::from_secs(11)).await;

    let err = starting.await.unwrap().unwrap_err();

    assert_eq!(err.code, code::CONFLICT);
    let after = f
        .tree()
        .await
        .into_iter()
        .find(|n| n.id == door.id)
        .unwrap();
    assert!(after.can_resume);
    assert_eq!(after.terminal_id, None);
}

#[tokio::test]
async fn a23_an_earlier_attempts_signal_is_ignored_by_a_restarted_door() {
    let f = Fixture::running("sleep 30");
    let door = exited_door(&f).await;
    let f = Arc::new(f);
    let old_attempt = door.attempt.clone().unwrap();
    let starting = {
        let (f, id) = (Arc::clone(&f), door.id.clone());
        tokio::spawn(async move { start_door(&f, &id).await })
    };
    f.until_attempt_changed(&door.id, &old_attempt).await;

    let stale = f
        .signal_payload(
            &door.id,
            &old_attempt,
            json!({"hook_event_name": "SessionStart", "session_id": OTHER_CONVERSATION, "source": "resume"}),
        )
        .await;
    assert!(stale.is_ok(), "ignored, never an error to the caller");
    f.session_start(&door.id, CONVERSATION, "resume")
        .await
        .unwrap();
    let restarted = starting.await.unwrap().unwrap();

    assert_eq!(restarted.status.as_ref().map(|s| s.kind), Some(Kind::Idle));
    assert!(restarted.terminal_id.is_some());
}
