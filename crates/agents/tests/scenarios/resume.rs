//! A21 saving an Agent's conversation, and A22 resuming it.

use std::sync::Arc;
use std::time::Duration;

use contracts::Kind;
use contracts::agent::RailNode;
use rpc::code;
use serde_json::{Value, json};

use crate::common::Fixture;

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
    pub async fn until_attempt_changed(&self, id: &str, old: &str) -> Vec<RailNode> {
        self.until(|tree| {
            tree.iter()
                .find(|n| n.id == id)
                .and_then(|n| n.attempt.as_deref())
                != Some(old)
        })
        .await
    }
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
async fn a21_a_failed_save_while_parked_fails_the_whole_spawn() {
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
    // Reserve the node before installing write failures, so only save/cleanup fail.
    let id = f
        .until(|tree| !tree.is_empty())
        .await
        .first()
        .map(|n| n.id.clone())
        .expect("the node exists while its launch is parked");
    let db = rusqlite::Connection::open(f.dir.path().join("agents.db")).unwrap();
    db.execute_batch("CREATE TRIGGER fail_save BEFORE UPDATE OF conversation_id ON nodes BEGIN SELECT RAISE(ABORT,'save refused'); END; CREATE TRIGGER fail_delete BEFORE DELETE ON nodes BEGIN SELECT RAISE(ABORT,'delete refused'); END;").unwrap();
    // The parked SessionStart reaches `agent.signal` before the config read `start` is waiting on
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

    db.execute_batch("DROP TRIGGER fail_save; DROP TRIGGER fail_delete;")
        .unwrap();
    assert!(
        result.is_err(),
        "a parked conversation save failure must fail the spawn"
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        while f.terminals.list().iter().any(|t| t.running) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("failed parked save must reap its actual Terminal");
    // Node deletion was also refused. The process-free registration test proves
    // reinsertion for caller cleanup; this observer proves the real Terminal is reaped.
    assert_eq!(
        f.tree().await.len(),
        1,
        "exactly the orphaned node remains, refused its own cleanup"
    );
}

#[tokio::test]
async fn a22_resume_launches_with_resume_flag_and_promotes_on_ack() {
    let f = Fixture::running("echo \"$@\" > argv.txt; sleep 30");
    let node = exited_with_conversation(&f, CONVERSATION).await;
    let old_facts =
        f.call("project.measurements", Value::Null).await.unwrap()["time_evidence"].clone();
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
    assert_eq!(
        f.call("project.measurements", Value::Null).await.unwrap()["time_evidence"],
        old_facts,
        "unacknowledged resume supplies no lifetime or execution"
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
    let read = f.call("project.measurements", Value::Null).await.unwrap();
    let attempts: Vec<_> = read["time_evidence"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|fact| fact["value"]["activity"]["kind"] == "attempt")
        .collect();
    assert_eq!(attempts.len(), 2);
    assert!(
        read["time_evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|fact| fact == &old_facts[0])
    );
    assert!(
        read["time_evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |fact| fact["attempt"].as_str() == resumed.attempt.as_deref()
                    && fact["value"]["state"]["kind"] == "active"
            )
    );
    let argv = until_resumed(&f).await;
    assert!(argv.contains(&format!("--resume {CONVERSATION}")));
}

#[tokio::test]
async fn a22_an_id_that_is_not_an_agent_is_conflict() {
    let f = Fixture::running("sleep 30");
    let workstream = f.workstream("r", None).await;

    let err = f.resume(&workstream).await.unwrap_err();

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
    let facts_before = f.call("project.measurements", Value::Null).await.unwrap();
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
    let facts_after = f.call("project.measurements", Value::Null).await.unwrap();
    assert_eq!(facts_after["time_evidence"], facts_before["time_evidence"]);
    assert_eq!(facts_after["contributions"], facts_before["contributions"]);
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

#[tokio::test]
async fn a22_no_acknowledgement_within_ten_seconds_times_out_and_rolls_back() {
    let f = Fixture::running("sleep 30");
    let node = exited_with_conversation(&f, CONVERSATION).await;
    // Plugin compatibility probes use a real process; pause only the resume deadline.
    tokio::time::pause();
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
    // Cleanup waits on the real PTY reader; its exit bound needs the real clock.
    tokio::time::resume();

    let err = resuming.await.unwrap().unwrap_err();

    assert_eq!(err.code, code::CONFLICT);
    let tree = f.tree().await;
    let after = tree.iter().find(|n| n.id == id).unwrap();
    assert!(after.can_resume);
    assert_eq!(after.terminal_id, None);
}

/// A `SessionStart` is the ack only with source `resume` naming the saved id; anything else is
/// parked exactly as a non-`SessionStart` Signal is, never promoting the pending resume. Proven by
/// timing it out (success would mean the mismatched ack was mistaken for the real one) rather
/// than by only sending the real one afterward, which would pass even if the check were a no-op.
async fn parked_as_not_the_ack(session_id: &str, source: &str) {
    let f = Fixture::running("sleep 30");
    let node = exited_with_conversation(&f, CONVERSATION).await;
    // Plugin compatibility probes use a real process; pause only the resume deadline.
    tokio::time::pause();
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
    // Cleanup waits on the real PTY reader; its exit bound needs the real clock.
    tokio::time::resume();

    let err = resuming.await.unwrap().unwrap_err();
    assert_eq!(err.code, code::CONFLICT, "{err}");
}

#[tokio::test]
async fn a22_an_ack_with_the_wrong_source_is_parked_not_promoted() {
    parked_as_not_the_ack(CONVERSATION, "startup").await;
}

#[tokio::test]
async fn a22_an_ack_naming_a_different_conversation_is_parked_not_promoted() {
    parked_as_not_the_ack(OTHER_CONVERSATION, "resume").await;
}

/// A non-ack parked first, then the real ack: proves a mismatch is parked (not rejected outright)
/// and the resume still resolves once the real one arrives.
#[tokio::test]
async fn a22_a_mismatched_signal_parked_before_the_real_ack_still_lets_it_resolve() {
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

/// A23: a Workstream whose Door started, saved `CONVERSATION` and was stopped.
async fn exited_door(f: &Fixture) -> RailNode {
    let workstream = f.workstream("team", None).await;
    f.call("rail.startDoor", json!({"id": workstream}))
        .await
        .unwrap();
    f.session_start(&workstream, CONVERSATION, "startup")
        .await
        .unwrap();
    f.call("agent.stop", json!({"id": workstream}))
        .await
        .unwrap();
    f.tree()
        .await
        .into_iter()
        .find(|n| n.id == workstream)
        .unwrap()
}

/// The fake `claude`'s recorded argv once a launch with `--resume` has written its line.
async fn until_resumed(f: &Fixture) -> String {
    for _ in 0..500 {
        let text = std::fs::read_to_string(f.dir.path().join("argv.txt")).unwrap_or_default();
        if text.split_inclusive('\n').any(|line| {
            line.ends_with('\n')
                && line
                    .split_whitespace()
                    .zip(line.split_whitespace().skip(1))
                    .any(|(flag, value)| flag == "--resume" && value == CONVERSATION)
        }) {
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
    let workstream = f.workstream("team", None).await;

    let door = start_door(&f, &workstream).await.unwrap();

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
    let argv = until_resumed(&f).await;
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

#[tokio::test]
async fn a23_a_restart_without_acknowledgement_fails_with_a22s_error() {
    let f = Fixture::running("sleep 30");
    let door = exited_door(&f).await;
    // Plugin compatibility probes use a real process; pause only the resume deadline.
    tokio::time::pause();
    let f = Arc::new(f);
    let old_attempt = door.attempt.clone().unwrap();
    let starting = {
        let (f, id) = (Arc::clone(&f), door.id.clone());
        tokio::spawn(async move { start_door(&f, &id).await })
    };
    f.until_attempt_changed(&door.id, &old_attempt).await;
    tokio::time::advance(Duration::from_secs(11)).await;
    // Cleanup waits on the real PTY reader; its exit bound needs the real clock.
    tokio::time::resume();

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

#[tokio::test]
async fn a24_a_resume_whose_run_is_gone_presents_a_live_status_revision() {
    let f = Fixture::running("sleep 30");
    let node = exited_with_conversation(&f, CONVERSATION).await;
    let f = Arc::new(f.reopen());
    let old_attempt = node.attempt.clone().unwrap();

    let resuming = {
        let f = Arc::clone(&f);
        let id = node.id.clone();
        tokio::spawn(async move { f.resume(&id).await })
    };
    let tree = f.until_attempt_changed(&node.id, &old_attempt).await;
    let pending = tree.iter().find(|n| n.id == node.id).unwrap();

    assert!(
        pending.status_revision.is_some(),
        "a pending resume is not a permanently ended node"
    );
    assert_ne!(pending.status.as_ref().map(|s| s.kind), Some(Kind::Done));
    f.session_start(&node.id, CONVERSATION, "resume")
        .await
        .unwrap();
    resuming.await.unwrap().unwrap();
}

#[tokio::test]
async fn r16_resume_availability_is_gated_by_the_current_attempt() {
    let f = Arc::new(Fixture::running("sleep 30"));
    let node = exited_with_conversation(&f, CONVERSATION).await;
    let old = node.attempt.clone().unwrap();
    assert!(node.can_resume);
    let resuming = {
        let f = Arc::clone(&f);
        let id = node.id.clone();
        tokio::spawn(async move { f.resume(&id).await })
    };
    let tree = f.until_attempt_changed(&node.id, &old).await;
    let pending = tree.iter().find(|n| n.id == node.id).unwrap();
    assert!(!pending.can_resume);
    assert_eq!(pending.status_revision.as_deref(), Some("1"));
    assert_eq!(pending.status.as_ref().unwrap().label, "starting");
    f.session_start(&node.id, CONVERSATION, "resume")
        .await
        .unwrap();
    let running = resuming.await.unwrap().unwrap();
    assert!(!running.can_resume);
    assert_eq!(
        f.tree()
            .await
            .into_iter()
            .find(|n| n.id == node.id)
            .unwrap(),
        running
    );
}

#[tokio::test]
async fn r16_reopened_rail_keeps_resume_data_without_live_presentation() {
    let f = Fixture::running("sleep 30");
    let node = exited_with_conversation(&f, CONVERSATION).await;
    let f = f.reopen();
    let reopened = f
        .tree()
        .await
        .into_iter()
        .find(|n| n.id == node.id)
        .unwrap();
    assert!(reopened.can_resume);
    assert_eq!(reopened.attempt, node.attempt);
    assert_eq!(reopened.status.as_ref().unwrap().kind, Kind::Done);
    assert!(reopened.status_revision.is_none());
    assert!(reopened.terminal_id.is_none());
    assert!(reopened.channel.is_none());
    assert!(reopened.stray.is_empty());
    assert_eq!(reopened.work, node.work);
    assert_eq!(reopened.model, node.model);
    assert_eq!(reopened.effort, node.effort);
    assert_eq!(reopened.iteration, node.iteration);
}

#[tokio::test]
async fn a2693a_missing_data_does_not_stop_the_target() {
    let f = Fixture::running("sleep 30");
    let node = f.spawn(None, None).await.unwrap();
    let err = f
        .call("agent.resume", json!({"id": node.id, "mode": "yolo"}))
        .await
        .unwrap_err();
    assert_eq!(err.code, code::CONFLICT);
    let terminal = f
        .terminals
        .list()
        .into_iter()
        .find(|t| Some(&t.id) == node.terminal_id.as_ref())
        .unwrap();
    assert!(terminal.running);
}

#[tokio::test]
async fn a2693a_yolo_argv_and_node_agree_after_ack() {
    let f = Arc::new(Fixture::running("echo \"$@\" > argv.txt; sleep 30"));
    let node = exited_with_conversation(&f, CONVERSATION).await;
    let resuming = {
        let f = Arc::clone(&f);
        let id = node.id.clone();
        tokio::spawn(async move {
            f.call("agent.resume", json!({"id": id, "mode": "yolo"}))
                .await
        })
    };
    let pending = f
        .until_attempt_changed(&node.id, node.attempt.as_deref().unwrap())
        .await;
    assert!(!pending.iter().find(|n| n.id == node.id).unwrap().yolo);
    f.session_start(&node.id, CONVERSATION, "resume")
        .await
        .unwrap();
    let resumed = resuming.await.unwrap().unwrap();
    assert_eq!(resumed["yolo"], true);
    let argv = until_resumed(&f).await;
    assert!(argv.contains(&format!("--resume {CONVERSATION}")));
    assert!(argv.contains("--dangerously-skip-permissions"));
    let tree = f.call("rail.tree", Value::Null).await.unwrap();
    assert_eq!(
        tree.as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == node.id)
            .unwrap()["yolo"],
        true
    );
}

async fn resume_in_mode(f: &Arc<Fixture>, node: &RailNode, mode: Option<&str>) -> RailNode {
    let pending = {
        let f = Arc::clone(f);
        let id = node.id.clone();
        let mode = mode.map(str::to_owned);
        tokio::spawn(async move {
            let mut params = json!({"id": id});
            if let Some(mode) = mode {
                params["mode"] = json!(mode);
            }
            f.call("agent.resume", params).await
        })
    };
    f.until_attempt_changed(&node.id, node.attempt.as_deref().unwrap())
        .await;
    f.session_start(&node.id, CONVERSATION, "resume")
        .await
        .unwrap();
    serde_json::from_value(pending.await.unwrap().unwrap()).unwrap()
}

#[tokio::test]
async fn a2693a_running_and_exited_targets_continue_once_in_each_mode() {
    for door in [false, true] {
        for running in [false, true] {
            for mode in ["normal", "yolo"] {
                let f = Arc::new(Fixture::running(
                    r#"here=$(dirname "$0"); if [ -f "$here/live.pid" ] && kill -0 "$(cat "$here/live.pid")" 2>/dev/null; then echo overlap > "$here/overlap"; fi; echo $$ > "$here/live.pid"; echo "$@" > "$here/argv.txt"; pwd > "$here/cwd.txt"; sleep 30"#,
                ));
                let node = if door {
                    let id = f.workstream("work", None).await;
                    serde_json::from_value(
                        f.call("rail.startDoor", json!({"id": id})).await.unwrap(),
                    )
                    .unwrap()
                } else {
                    f.spawn(None, None).await.unwrap()
                };
                crate::common::until_file(&f.dir.path().join("argv.txt")).await;
                let original_cwd = crate::common::until_file(&f.dir.path().join("cwd.txt")).await;
                f.session_start(&node.id, CONVERSATION, "startup")
                    .await
                    .unwrap();
                if !running {
                    f.call("agent.stop", json!({"id":node.id})).await.unwrap();
                }
                let resumed = resume_in_mode(&f, &node, Some(mode)).await;
                assert_eq!(resumed.id, node.id);
                assert_eq!(resumed.name, node.name);
                assert_eq!(resumed.parent, node.parent);
                assert_eq!(resumed.order, node.order);
                assert_eq!(resumed.worktree, node.worktree);
                assert_ne!(resumed.terminal_id, node.terminal_id);
                let terminals = f.terminals.list();
                assert_eq!(terminals.len(), 2, "exactly one next launch");
                assert_eq!(terminals.iter().filter(|t| t.running).count(), 1);
                assert!(
                    !terminals
                        .iter()
                        .find(|t| Some(&t.id) == node.terminal_id.as_ref())
                        .unwrap()
                        .running
                );
                let argv = until_resumed(&f).await;
                assert!(argv.contains(&format!("--resume {CONVERSATION}")));
                assert!(
                    !f.dir.path().join("overlap").exists(),
                    "new launch preceded old exit"
                );
                assert_eq!(
                    std::fs::read_to_string(f.dir.path().join("cwd.txt")).unwrap(),
                    original_cwd
                );
            }
        }
    }
}

#[tokio::test]
async fn a2693a_every_normal_launch_clears_yolo() {
    for mode in [None, Some("normal"), Some("door")] {
        let f = Arc::new(Fixture::running(
            "echo \"$@\" > \"$(dirname \"$0\")/argv.txt\"; sleep 30",
        ));
        let node = if mode == Some("door") {
            exited_door(&f).await
        } else {
            exited_with_conversation(&f, CONVERSATION).await
        };
        let yolo = resume_in_mode(&f, &node, Some("yolo")).await;
        until_resumed(&f).await;
        f.call("agent.stop", json!({"id":node.id})).await.unwrap();
        std::fs::remove_file(f.dir.path().join("argv.txt")).unwrap();
        let normal = if mode == Some("door") {
            let starting = {
                let f = Arc::clone(&f);
                let id = node.id.clone();
                tokio::spawn(async move { start_door(&f, &id).await })
            };
            f.until_attempt_changed(&node.id, yolo.attempt.as_deref().unwrap())
                .await;
            f.session_start(&node.id, CONVERSATION, "resume")
                .await
                .unwrap();
            starting.await.unwrap().unwrap()
        } else {
            resume_in_mode(&f, &yolo, mode).await
        };
        assert!(!normal.yolo);
        assert!(
            !until_resumed(&f)
                .await
                .contains("--dangerously-skip-permissions")
        );
    }
}

#[tokio::test]
async fn a2693a_concurrent_actions_never_launch_twice() {
    let f = Arc::new(Fixture::running("sleep 30"));
    let node = f.spawn(None, None).await.unwrap();
    f.session_start(&node.id, CONVERSATION, "startup")
        .await
        .unwrap();
    let pending = {
        let f = Arc::clone(&f);
        let id = node.id.clone();
        tokio::spawn(async move { f.call("agent.resume", json!({"id":id,"mode":"yolo"})).await })
    };
    f.until_attempt_changed(&node.id, node.attempt.as_deref().unwrap())
        .await;
    for method in ["agent.resume", "agent.stop", "rail.remove"] {
        let err = f
            .call(method, json!({"id":node.id,"mode":"normal"}))
            .await
            .unwrap_err();
        assert_eq!(err.code, code::CONFLICT, "{method}");
    }
    f.session_start(&node.id, CONVERSATION, "resume")
        .await
        .unwrap();
    pending.await.unwrap().unwrap();
    assert_eq!(f.terminals.list().iter().filter(|t| t.running).count(), 1);
}

#[tokio::test]
async fn a2693a_exit_keeps_and_reopen_clears_the_attempt_mode() {
    let f = Arc::new(Fixture::running("sleep 30"));
    let node = exited_with_conversation(&f, CONVERSATION).await;
    let yolo = resume_in_mode(&f, &node, Some("yolo")).await;
    f.call("agent.stop", json!({"id":yolo.id})).await.unwrap();
    assert!(
        f.tree()
            .await
            .iter()
            .find(|n| n.id == node.id)
            .unwrap()
            .yolo
    );
    let f = Arc::try_unwrap(f).ok().unwrap().reopen();
    assert!(
        !f.tree()
            .await
            .iter()
            .find(|n| n.id == node.id)
            .unwrap()
            .yolo
    );
}

#[tokio::test]
async fn a2693a_failed_start_does_not_publish_the_requested_mode() {
    let f = Arc::new(Fixture::running("sleep 30"));
    let node = exited_with_conversation(&f, CONVERSATION).await;
    std::fs::remove_file(f.dir.path().join("fake-claude")).unwrap();
    let err = f
        .call("agent.resume", json!({"id":node.id,"mode":"yolo"}))
        .await
        .unwrap_err();
    assert!(!err.message.is_empty());
    let after = f
        .tree()
        .await
        .into_iter()
        .find(|n| n.id == node.id)
        .unwrap();
    assert!(!after.yolo);
    assert_eq!(after.terminal_id, node.terminal_id);
}

#[tokio::test]
async fn b2693b_note_mode_matches_the_receiver_attempt() {
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixListener;
    let f = Arc::new(Fixture::running("echo \"$@\" > argv.txt; sleep 30"));
    let node = exited_with_conversation(&f, CONVERSATION).await;
    let yolo = resume_in_mode(&f, &node, Some("yolo")).await;
    let argv = until_resumed(&f).await;
    let socket = argv
        .split_whitespace()
        .skip_while(|word| *word != "--messaging-socket-path")
        .nth(1)
        .unwrap();
    let mut listener = UnixListener::bind(socket).unwrap();
    let sessions = f.dir.path().join("sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    let pid = std::process::id();
    std::fs::write(
        sessions.join(format!("{pid}.json")),
        json!({"pid":pid,"messagingSocketPath":socket,"peerProtocol":1}).to_string(),
    )
    .unwrap();
    std::fs::write(
        sessions.join(format!("{pid}.test.key")),
        json!({"peerToken":"test-token"}).to_string(),
    )
    .unwrap();
    for (sender, expected) in [
        ("normal sender", "bypassPermissions"),
        ("YOLO sender", "default"),
    ] {
        if expected == "default" {
            resume_in_mode(&f, &yolo, Some("normal")).await;
            drop(listener);
            listener = UnixListener::bind(socket).unwrap();
        }
        f.agents
            .inbox_write(
                &node.id,
                &[agents::claude_code::inbox::Entry {
                    id: 1,
                    actor: contracts::Actor::user(),
                    kind: contracts::message::MessageKind::Note,
                    from: sender.into(),
                    text: "note".into(),
                    one_line: false,
                }],
            )
            .unwrap();
        let (stream, _) = listener.accept().unwrap();
        let lines: Vec<Value> = BufReader::new(stream)
            .lines()
            .map(|line| serde_json::from_str(&line.unwrap()).unwrap())
            .collect();
        assert!(
            lines[1]["message"]["content"]
                .as_str()
                .unwrap()
                .contains(&format!("from-mode=\"{expected}\""))
        );
    }
}

#[tokio::test]
async fn a2693a_failed_resume_keeps_the_old_node() {
    let mut f = Fixture::running("echo old snapshot; sleep 30");
    f.agents = f.agents.with_resume_effect(|phase, _| {
        if phase == "claimed" {
            Err(rpc::RpcError::conflict("resume rejected"))
        } else {
            Ok(())
        }
    });
    let node = f.spawn(None, None).await.unwrap();
    f.session_start(&node.id, CONVERSATION, "startup")
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while f
            .terminals
            .snapshot(node.terminal_id.as_deref().unwrap())
            .unwrap()
            .after
            == 0
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut exit = f
        .terminals
        .subscribe(node.terminal_id.as_deref().unwrap())
        .unwrap();
    f.call("agent.stop", json!({"id":node.id})).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while !matches!(
            exit.recv().await.unwrap().as_ref(),
            contracts::EventData::TerminalExited(_)
        ) {}
    })
    .await
    .unwrap();
    let old_node = f
        .tree()
        .await
        .into_iter()
        .find(|n| n.id == node.id)
        .unwrap();
    let before = f
        .terminals
        .snapshot(node.terminal_id.as_deref().unwrap())
        .unwrap();
    let err = f
        .call("agent.resume", json!({"id":node.id,"mode":"yolo"}))
        .await
        .unwrap_err();
    assert!(err.message.contains("resume rejected"));
    let after = f
        .tree()
        .await
        .into_iter()
        .find(|n| n.id == node.id)
        .unwrap();
    assert_eq!(after.terminal_id, old_node.terminal_id);
    assert_eq!(after.name, node.name);
    assert!(!after.yolo);
    assert!(f.terminals.list().iter().all(|t| !t.running));
    assert_eq!(
        f.terminals
            .snapshot(node.terminal_id.as_deref().unwrap())
            .unwrap()
            .data,
        before.data
    );
}

#[tokio::test]
async fn a2693a_cleanup_stop_keeps_the_last_acknowledged_mode() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let fail = Arc::new(AtomicBool::new(false));
    let mut f = Fixture::running("sleep 30");
    f.agents = f.agents.with_resume_effect({
        let fail = Arc::clone(&fail);
        move |phase, _| {
            if fail.load(Ordering::SeqCst) && matches!(phase, "claimed" | "cleanup") {
                Err(rpc::RpcError::conflict("injected resume/cleanup failure"))
            } else {
                Ok(())
            }
        }
    });
    let f = Arc::new(f);
    let node = exited_with_conversation(&f, CONVERSATION).await;
    resume_in_mode(&f, &node, Some("yolo")).await;
    fail.store(true, Ordering::SeqCst);
    let err = f
        .call("agent.resume", json!({"id":node.id,"mode":"normal"}))
        .await
        .unwrap_err();
    assert_eq!(err.code, code::UNKNOWN_OUTCOME);
    assert!(
        f.tree()
            .await
            .iter()
            .find(|n| n.id == node.id)
            .unwrap()
            .yolo
    );
    fail.store(false, Ordering::SeqCst);
    f.call("agent.stop", json!({"id":node.id})).await.unwrap();
    assert!(
        f.tree()
            .await
            .iter()
            .find(|n| n.id == node.id)
            .unwrap()
            .yolo
    );
}

fn overview_transcript(f: &Fixture, text: &str) -> std::path::PathBuf {
    let cwd = f.dir.path().canonicalize().unwrap();
    let project: String = cwd
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let path = f
        .dir
        .path()
        .join("projects")
        .join(project)
        .join(format!("{CONVERSATION}.jsonl"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path
}

#[tokio::test]
async fn a2681a_closed_node_has_identity_without_history_path() {
    let f = Fixture::running("sleep 30");
    let path = overview_transcript(
        &f,
        "{\"type\":\"ai-title\",\"aiTitle\":\"A conversation\"}\n",
    );
    let node = exited_with_conversation(&f, CONVERSATION).await;
    let tree = f.call("rail.tree", json!({})).await.unwrap();
    let node = tree
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == node.id)
        .unwrap();
    assert_eq!(
        node["overview"],
        json!({"program": "Claude Code", "id": CONVERSATION, "name": "A conversation"})
    );
    assert!(!tree.to_string().contains(path.to_str().unwrap()));
    assert!(!tree.to_string().contains("aiTitle"));
}

#[tokio::test]
async fn a2681a_missing_or_unreadable_transcript_has_no_parts() {
    for unreadable in [false, true] {
        let f = Fixture::running("sleep 30");
        let path = overview_transcript(&f, "{}\n");
        std::fs::remove_file(&path).unwrap();
        if unreadable {
            std::fs::create_dir(path).unwrap();
        }
        let node = exited_with_conversation(&f, CONVERSATION).await;
        assert!(node.overview.is_none());
    }
}

#[tokio::test]
async fn a2681a_no_saved_data_has_no_parts() {
    let f = Fixture::running("sleep 30");
    overview_transcript(&f, "{\"type\":\"ai-title\",\"aiTitle\":\"Do not infer\"}\n");
    let node = f.spawn(None, None).await.unwrap();
    f.call("agent.stop", json!({"id":node.id})).await.unwrap();
    assert!(
        f.tree()
            .await
            .iter()
            .find(|n| n.id == node.id)
            .unwrap()
            .overview
            .is_none()
    );
}

#[tokio::test]
async fn a2681a_reopen_reads_closed_parts() {
    let f = Fixture::running("sleep 30");
    overview_transcript(&f, "{\"type\":\"ai-title\",\"aiTitle\":\"Kept\"}\n");
    let node = exited_with_conversation(&f, CONVERSATION).await;
    let f = f.reopen();
    let tree = f.tree().await;
    let reopened = tree.iter().find(|n| n.id == node.id).unwrap();
    assert_eq!(reopened.overview, node.overview);
    assert!(reopened.overview.is_some());
    assert!(reopened.terminal_id.is_none());
}

#[tokio::test]
async fn a2681a_live_attempt_hides_parts_including_on_resume() {
    let f = Fixture::running("sleep 30");
    overview_transcript(
        &f,
        "{\"type\":\"ai-title\",\"aiTitle\":\"Hidden while live\"}\n",
    );
    let node = f.spawn(None, None).await.unwrap();
    f.session_start(&node.id, CONVERSATION, "startup")
        .await
        .unwrap();
    assert!(
        f.tree()
            .await
            .iter()
            .find(|n| n.id == node.id)
            .unwrap()
            .overview
            .is_none()
    );
    f.call("agent.stop", json!({"id":node.id})).await.unwrap();
    let closed = f
        .until(|tree| tree.iter().any(|n| n.id == node.id && n.can_resume))
        .await;
    assert!(
        closed
            .iter()
            .find(|n| n.id == node.id)
            .unwrap()
            .overview
            .is_some()
    );
    let f = Arc::new(f);
    let resuming = {
        let f = Arc::clone(&f);
        let id = node.id.clone();
        tokio::spawn(async move { f.resume(&id).await })
    };
    let pending = f
        .until_attempt_changed(&node.id, node.attempt.as_deref().unwrap())
        .await;
    assert!(
        pending
            .iter()
            .find(|n| n.id == node.id)
            .unwrap()
            .overview
            .is_none()
    );
    f.session_start(&node.id, CONVERSATION, "resume")
        .await
        .unwrap();
    let resumed = resuming.await.unwrap().unwrap();
    assert_eq!(resumed.status.as_ref().unwrap().kind, Kind::Idle);
    assert!(resumed.overview.is_none(), "an idle program is live");
    f.call("agent.stop", json!({"id":node.id})).await.unwrap();
}

#[tokio::test]
async fn a2681d_live_attempt_hides_context_parts() {
    let f = Fixture::running("sleep 30");
    let context = include_str!("../fixtures/overview/2.1.296-context.jsonl");
    overview_transcript(
        &f,
        &format!(
            "{{\"type\":\"assistant\",\"message\":{{\"usage\":{{\"input_tokens\":20000}}}}}}\n{}\n",
            context.trim()
        ),
    );
    let node = f.spawn(None, None).await.unwrap();
    f.session_start(&node.id, CONVERSATION, "startup")
        .await
        .unwrap();
    assert!(
        f.tree()
            .await
            .iter()
            .find(|n| n.id == node.id)
            .unwrap()
            .overview
            .is_none()
    );
    f.call("agent.stop", json!({"id":node.id})).await.unwrap();
    let closed = f
        .until(|tree| tree.iter().any(|n| n.id == node.id && n.can_resume))
        .await;
    let overview = closed
        .iter()
        .find(|n| n.id == node.id)
        .unwrap()
        .overview
        .as_ref()
        .unwrap();
    assert!(overview.context_parts.is_some());
}

#[tokio::test]
async fn a2681a_natural_exit_exposes_parts() {
    let f = Fixture::running("while [ ! -f release ]; do sleep 0.01; done");
    overview_transcript(&f, "{}\n");
    let node = f.spawn(None, None).await.unwrap();
    f.session_start(&node.id, CONVERSATION, "startup")
        .await
        .unwrap();
    std::fs::write(f.dir.path().join("release"), "").unwrap();
    let tree = f
        .until(|tree| tree.iter().any(|n| n.id == node.id && n.can_resume))
        .await;
    assert!(
        tree.iter()
            .find(|n| n.id == node.id)
            .unwrap()
            .overview
            .is_some()
    );
}

#[tokio::test]
async fn u3016c_elapsed_and_parallel_time_are_distinct_sources() {
    use std::sync::atomic::{AtomicU64, Ordering};
    let now = Arc::new(AtomicU64::new(0));
    let (claimed, mut claims) = tokio::sync::mpsc::unbounded_channel();
    let mut f = Fixture::running("sleep 30");
    let wall = now.clone();
    let mono = now.clone();
    f.agents = f
        .agents
        .with_clock(move || i64::try_from(wall.load(Ordering::SeqCst)).unwrap())
        .with_measurement_clock(move || u128::from(mono.load(Ordering::SeqCst)) * 1_000_000)
        .with_resume_effect(move |phase, _| {
            if phase == "claimed" {
                claimed.send(()).unwrap();
            }
            Ok(())
        });
    let door = f.workstream("door", None).await;
    f.call("rail.startDoor", json!({"id":door})).await.unwrap();
    f.session_start(&door, CONVERSATION, "startup")
        .await
        .unwrap();
    let child = f.spawn(Some(&door), None).await.unwrap();
    f.session_start(&child.id, OTHER_CONVERSATION, "startup")
        .await
        .unwrap();
    for sec in 1..=3 {
        now.store(sec * 1000, Ordering::SeqCst);
        f.call("project.measurements", Value::Null).await.unwrap();
    }
    f.call("agent.stop", json!({"id":door})).await.unwrap();
    let stopped = f.call("project.measurements", Value::Null).await.unwrap();
    let facts: Vec<_> = stopped["time_evidence"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|fact| fact["value"]["activity"]["kind"] == "attempt")
        .collect();
    assert_eq!(facts.len(), 2);
    let door_fact = facts.iter().find(|fact| fact["agent"] == door).unwrap();
    assert_eq!(door_fact["value"]["start"]["unix_ms"], 0);
    assert_eq!(door_fact["value"]["state"]["at"]["unix_ms"], 3000);
    assert!(
        facts
            .iter()
            .any(|fact| fact["agent"] == child.id && fact["value"]["state"]["kind"] == "active")
    );
    for sec in 4..=5 {
        now.store(sec * 1000, Ordering::SeqCst);
        f.call("project.measurements", Value::Null).await.unwrap();
    }
    f.call("agent.stop", json!({"id":child.id})).await.unwrap();
    let ended = f.call("project.measurements", Value::Null).await.unwrap();
    now.store(10000, Ordering::SeqCst);
    assert_eq!(
        f.call("project.measurements", Value::Null).await.unwrap()["time_evidence"],
        ended["time_evidence"],
        "the idle gap extends no lifetime"
    );
    let f = Arc::new(f);
    let resuming = {
        let f = f.clone();
        let door = door.clone();
        tokio::spawn(async move { f.call("rail.startDoor", json!({"id":door})).await })
    };
    let old = door_fact["attempt"].as_str().unwrap();
    f.until_attempt_changed(&door, old).await;
    assert_eq!(
        f.call("project.measurements", Value::Null).await.unwrap()["time_evidence"],
        ended["time_evidence"]
    );
    claims.recv().await.unwrap();
    now.store(11000, Ordering::SeqCst);
    f.session_start(&door, CONVERSATION, "resume")
        .await
        .unwrap();
    let resumed: RailNode = serde_json::from_value(resuming.await.unwrap().unwrap()).unwrap();
    let active = f.call("project.measurements", Value::Null).await.unwrap();
    assert_eq!(
        active["time_evidence"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|fact| fact["value"]["activity"]["kind"] == "attempt")
            .count(),
        3
    );
    let latest = active["time_evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fact| {
            fact["agent"] == door
                && fact["attempt"].as_str() == resumed.attempt.as_deref()
                && fact["value"]["activity"]["kind"] == "attempt"
        })
        .unwrap();
    assert_eq!(
        latest["value"]["start"]["unix_ms"], 10000,
        "the observed launch precedes acknowledgement"
    );
    now.store(12000, Ordering::SeqCst);
    f.call("agent.stop", json!({"id":door})).await.unwrap();
    let before = f.call("project.measurements", Value::Null).await.unwrap();
    let f = Arc::try_unwrap(f).ok().unwrap().reopen();
    assert_eq!(
        f.call("project.measurements", Value::Null).await.unwrap()["time_evidence"],
        before["time_evidence"]
    );
    std::fs::remove_file(f.dir.path().join("fake-claude")).unwrap();
    assert!(f.call("rail.startDoor", json!({"id":door})).await.is_err());
    let failed = f.call("project.measurements", Value::Null).await.unwrap();
    assert_eq!(
        failed["time_evidence"], before["time_evidence"],
        "failed resume does not replace old facts"
    );
    assert!(
        failed["contributions"].as_array().unwrap().is_empty(),
        "an unacknowledged launch is no model/tool evidence"
    );
}

#[tokio::test]
async fn u3016c_live_resume_ends_the_replaced_attempt() {
    use std::sync::atomic::{AtomicU64, Ordering};
    let now = Arc::new(AtomicU64::new(0));
    let mut f = Fixture::running("sleep 30");
    let wall = now.clone();
    let mono = now.clone();
    f.agents = f
        .agents
        .with_clock(move || i64::try_from(wall.load(Ordering::SeqCst)).unwrap())
        .with_measurement_clock(move || u128::from(mono.load(Ordering::SeqCst)) * 1_000_000);
    let f = Arc::new(f);
    let node = f.spawn(None, None).await.unwrap();
    f.session_start(&node.id, CONVERSATION, "startup")
        .await
        .unwrap();
    now.store(1000, Ordering::SeqCst);
    let resumed = resume_in_mode(&f, &node, Some("normal")).await;
    let after_resume = f.call("project.measurements", Value::Null).await.unwrap();
    let old = after_resume["time_evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fact| fact["attempt"].as_str() == node.attempt.as_deref())
        .unwrap();
    assert_eq!(old["value"]["state"]["kind"], "ended");
    assert_eq!(old["value"]["state"]["at"]["unix_ms"], 1000);
    now.store(2000, Ordering::SeqCst);
    f.call("agent.stop", json!({"id":resumed.id}))
        .await
        .unwrap();
    let stopped = f.call("project.measurements", Value::Null).await.unwrap();
    assert_eq!(
        stopped["time_evidence"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fact| fact["attempt"].as_str() == node.attempt.as_deref())
            .unwrap(),
        old,
        "the replacement cannot extend the old Attempt"
    );
    now.store(3000, Ordering::SeqCst);
    assert_eq!(
        f.call("project.measurements", Value::Null).await.unwrap()["time_evidence"],
        stopped["time_evidence"],
        "stopping the replacement leaves no active lifetime"
    );
}
