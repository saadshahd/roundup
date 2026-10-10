//! O1 to O5: the order every Agent and Door holds. The fake `claude` records every byte typed to
//! its Terminal (as `steer.rs` does) and the test plays its hooks with `agent.signal`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use contracts::agent::{NodeKind, Order, RailNode};
use contracts::{Actor, ActorKind, Verb};
use provenance::Touches;
use rpc::{Ctx, Module, RpcError, code};
use serde_json::{Value, json};

use crate::common::{Fixture, GATED, star, until_file};

const RECORD: &str = GATED;

fn paste(text: &str) -> String {
    format!("\x1b[200~{text}\x1b[201~\r")
}

fn recording() -> Fixture {
    let mut f = Fixture::running(RECORD);
    f.agents = f.agents.with_clock(|| 0);
    f
}

fn typed(f: &Fixture) -> String {
    let bytes = std::fs::read(f.dir.path().join("typed")).unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

async fn until_typed(f: &Fixture, len: usize) -> String {
    for _ in 0..500 {
        let text = typed(f);
        if text.len() >= len {
            return text;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("{len} bytes were never typed: {:?}", typed(f));
}

async fn signal(f: &Fixture, id: &str, event: &str) {
    let attempt = f.attempt(id).await;
    f.call(
        "agent.signal",
        json!({"id": id, "attempt": attempt, "payload": {"hook_event_name": event}}),
    )
    .await
    .unwrap();
}

async fn spawn(f: &Fixture, extra: Value) -> Result<RailNode, RpcError> {
    let mut params = json!({"cwd": f.dir.path(), "parent": null});
    params
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    let node = f.call("agent.spawn", params).await?;
    Ok(serde_json::from_value(node).unwrap())
}

fn work(ask: &str, limits: &[&str]) -> Order {
    Order::Work {
        ask: ask.into(),
        limits: limits.iter().map(|limit| (*limit).into()).collect(),
    }
}

fn order_of(tree: &[RailNode], id: &str) -> Option<Order> {
    tree.iter()
        .find(|n| n.id == id)
        .and_then(|n| n.work.clone())
}

fn from(actor: ActorKind, id: &str, f: &Fixture, touches: &Arc<Touches>) -> Ctx {
    Ctx {
        actor: Actor {
            kind: actor,
            id: id.into(),
            parent: None,
        },
        touches: Arc::clone(touches),
        ..f.ctx()
    }
}

#[tokio::test]
async fn o1_o2_a_spawn_holds_the_order_it_was_given_or_the_one_its_prompt_or_nothing_makes() {
    let f = Fixture::new();
    let given = work("fix the build", &["touch CI"]);
    let by_order = spawn(&f, json!({"order": given})).await.unwrap();
    let by_prompt = spawn(&f, json!({"prompt": "fix it"})).await.unwrap();
    let by_nothing = spawn(&f, json!({})).await.unwrap();
    let asking = Order::Clarification {
        question: "Which test?".into(),
    };
    let by_question = spawn(&f, json!({"order": asking})).await.unwrap();

    let tree = f.tree().await;
    assert_eq!(order_of(&tree, &by_order.id), Some(given));
    assert_eq!(order_of(&tree, &by_prompt.id), Some(work("fix it", &[])));
    assert_eq!(
        order_of(&tree, &by_nothing.id),
        Some(Order::clarification("What should this Agent do?"))
    );
    assert_eq!(order_of(&tree, &by_question.id), Some(asking));
}

#[tokio::test]
async fn o1_a_terminal_has_no_order_and_a_workstream_has_one() {
    let f = Fixture::new();
    let workstream = f.workstream("team", None).await;
    f.call(
        "rail.spawnTerminal",
        json!({"cwd": f.dir.path(), "parent": null}),
    )
    .await
    .unwrap();

    let tree = f.tree().await;
    for node in &tree {
        match node.kind {
            NodeKind::Terminal => assert_eq!(node.work, None),
            _ => assert!(node.work.is_some(), "{} has no order", node.id),
        }
    }
    assert_eq!(
        order_of(&tree, &workstream),
        Some(Order::clarification("What is this Workstream for?"))
    );
}

#[tokio::test]
async fn o2_a_prompt_with_an_order_or_a_malformed_order_is_invalid_and_changes_nothing() {
    let f = Fixture::new();
    let bad = [
        json!({"prompt": "go", "order": work("go", &[])}),
        json!({"order": work("  ", &[])}),
        json!({"order": work("go", &[" "])}),
        json!({"order": {"kind": "clarification", "question": ""}}),
        json!({"order": {"kind": "other"}}),
    ];
    for extra in bad {
        let err = spawn(&f, extra.clone()).await.unwrap_err();
        assert_eq!(err.code, code::INVALID_PARAMS, "{extra}");
    }
    assert!(f.tree().await.is_empty());
}

#[tokio::test]
async fn o2_the_first_steer_of_a_work_order_is_the_ask_then_its_limits() {
    let f = recording();
    let node = spawn(
        &f,
        json!({"order": work("fix the build", &["touch CI", "push"])}),
    )
    .await
    .unwrap();
    until_file(&f.dir.path().join("ready")).await;
    star(f.dir.path());
    signal(&f, &node.id, "SessionStart").await;

    let text = paste("fix the build\n\nMust not:\n- touch CI\n- push");
    assert_eq!(until_typed(&f, text.len()).await, text);
}

#[tokio::test]
async fn o2_the_first_steer_of_a_clarification_order_asks_to_find_elicit_and_record() {
    let f = recording();
    let order = Order::clarification("Which test fails?");
    let node = spawn(&f, json!({"order": order})).await.unwrap();
    until_file(&f.dir.path().join("ready")).await;
    star(f.dir.path());
    signal(&f, &node.id, "SessionStart").await;

    let steer = until_typed(&f, 100).await;
    assert!(steer.contains("Which test fails?"), "{steer}");
    assert!(steer.contains("agent_context"), "{steer}");
    assert!(steer.contains("agent_set_order"), "{steer}");
}

#[tokio::test]
async fn o3_a_workstream_gives_its_door_the_order_and_stopping_the_door_keeps_it() {
    let f = recording();
    let given = work("coordinate the release", &["merge"]);
    let workstream = f
        .call(
            "rail.createWorkstream",
            json!({"name": "release", "parent": null, "order": given}),
        )
        .await
        .unwrap();
    let id = workstream["id"].as_str().unwrap().to_owned();
    assert_eq!(order_of(&f.tree().await, &id), Some(given.clone()));

    f.call("rail.startDoor", json!({"id": id})).await.unwrap();
    until_file(&f.dir.path().join("ready")).await;
    star(f.dir.path());
    signal(&f, &id, "SessionStart").await;
    let text = paste("coordinate the release\n\nMust not:\n- merge");
    assert_eq!(until_typed(&f, text.len()).await, text);

    f.call("agent.stop", json!({"id": id})).await.unwrap();
    assert_eq!(order_of(&f.tree().await, &id), Some(given));

    let err = f
        .call(
            "rail.createWorkstream",
            json!({"name": "x", "parent": null, "order": work("", &[])}),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, code::INVALID_PARAMS);
}

#[tokio::test]
async fn o4_the_agent_its_door_or_the_user_may_set_an_order_and_it_is_a_touch() {
    let f = Fixture::new();
    let touches = Arc::new(Touches::in_memory().unwrap());
    let workstream = f.workstream("team", None).await;
    let agent = spawn(&f, json!({"parent": workstream})).await.unwrap().id;
    let stranger = spawn(&f, json!({})).await.unwrap().id;
    let set = |actor: Ctx, order: Order| {
        let (f, agent) = (&f, agent.clone());
        async move {
            f.agents
                .call(
                    &actor,
                    "agent.setOrder",
                    json!({"id": agent, "order": order}),
                )
                .await
        }
    };

    for (n, actor) in [
        from(ActorKind::Agent, &agent, &f, &touches),
        from(ActorKind::Agent, &workstream, &f, &touches),
        from(ActorKind::User, "you", &f, &touches),
    ]
    .into_iter()
    .enumerate()
    {
        let order = work(&format!("ask {n}"), &["a limit"]);
        let node = set(actor, order.clone()).await.unwrap();
        let node: RailNode = serde_json::from_value(node).unwrap();
        assert_eq!(node.work, Some(order.clone()));
        assert_eq!(order_of(&f.tree().await, &agent), Some(order));
    }

    let by: Vec<_> = touches
        .history(&format!("agent:{agent}"))
        .unwrap()
        .into_iter()
        .map(|touch| (touch.actor.id, touch.verb))
        .collect();
    assert_eq!(
        by,
        [
            (agent.clone(), Verb::Wrote),
            (workstream.clone(), Verb::Wrote),
            ("you".into(), Verb::Wrote)
        ]
    );

    let err = set(
        from(ActorKind::Agent, &stranger, &f, &touches),
        work("x", &[]),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, code::FORBIDDEN);
}

#[tokio::test]
async fn o4_unknown_terminal_and_malformed_are_refused_and_change_nothing() {
    let f = Fixture::new();
    let agent = spawn(&f, json!({"order": work("keep", &[])}))
        .await
        .unwrap()
        .id;
    let terminal = f
        .call(
            "rail.spawnTerminal",
            json!({"cwd": f.dir.path(), "parent": null}),
        )
        .await
        .unwrap();
    let terminal = terminal["id"].as_str().unwrap().to_owned();

    let cases = [
        (
            json!({"id": "999", "order": work("x", &[])}),
            code::NOT_FOUND,
        ),
        (
            json!({"id": terminal, "order": work("x", &[])}),
            code::CONFLICT,
        ),
        (
            json!({"id": agent, "order": work(" ", &[])}),
            code::INVALID_PARAMS,
        ),
        (
            json!({"id": agent, "order": {"kind": "x"}}),
            code::INVALID_PARAMS,
        ),
    ];
    for (params, expected) in cases {
        let err = f.call("agent.setOrder", params.clone()).await.unwrap_err();
        assert_eq!(err.code, expected, "{params}");
    }
    assert_eq!(order_of(&f.tree().await, &agent), Some(work("keep", &[])));
}

#[tokio::test]
async fn o4_another_callers_order_is_steered_unless_the_agent_set_it_or_a_takeover_holds() {
    let mut f = recording();
    let taken = Arc::new(AtomicBool::new(false));
    let probe = Arc::clone(&taken);
    f.agents = f
        .agents
        .with_takeover(move |_| probe.load(Ordering::SeqCst));
    let agent = spawn(&f, json!({"prompt": "first"})).await.unwrap().id;
    until_file(&f.dir.path().join("ready")).await;
    star(f.dir.path());
    signal(&f, &agent, "SessionStart").await;
    let first = paste("first");
    until_typed(&f, first.len()).await;
    signal(&f, &agent, "UserPromptSubmit").await;
    signal(&f, &agent, "Stop").await;
    f.until(|tree| {
        tree.iter().any(|n| {
            n.id == agent
                && n.status
                    .as_ref()
                    .is_some_and(|s| s.kind == contracts::Kind::Idle)
        })
    })
    .await;

    // The Agent's own change is not sent back to it.
    let touches = Arc::new(Touches::in_memory().unwrap());
    let own = from(ActorKind::Agent, &agent, &f, &touches);
    f.agents
        .call(
            &own,
            "agent.setOrder",
            json!({"id": agent, "order": work("mine", &[])}),
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(typed(&f), first);

    // Under a Takeover nothing is sent either, yet the order is stored.
    taken.store(true, Ordering::SeqCst);
    f.call(
        "agent.setOrder",
        json!({"id": agent, "order": work("held", &[])}),
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(typed(&f), first);
    assert_eq!(order_of(&f.tree().await, &agent), Some(work("held", &[])));

    taken.store(false, Ordering::SeqCst);
    f.call(
        "agent.setOrder",
        json!({"id": agent, "order": work("sent", &["x"])}),
    )
    .await
    .unwrap();
    let all = format!("{first}{}", paste("sent\n\nMust not:\n- x"));
    assert_eq!(until_typed(&f, all.len()).await, all);
}

#[tokio::test]
async fn o5_an_order_survives_a_reopen_and_a_move_and_leaves_with_its_node() {
    let f = Fixture::new();
    let given = work("keep me", &["leave"]);
    let workstream = f.workstream("team", None).await;
    let agent = spawn(&f, json!({"order": given})).await.unwrap().id;

    f.call(
        "rail.move",
        json!({"id": agent, "parent": workstream, "index": 0}),
    )
    .await
    .unwrap();
    assert_eq!(order_of(&f.tree().await, &agent), Some(given.clone()));

    let f = f.reopen();
    assert_eq!(order_of(&f.tree().await, &agent), Some(given));
    assert_eq!(
        order_of(&f.tree().await, &workstream),
        Some(Order::clarification("What is this Workstream for?"))
    );

    f.call("rail.remove", json!({"id": agent})).await.unwrap();
    assert_eq!(order_of(&f.tree().await, &agent), None);
}

#[tokio::test]
async fn a25_the_default_door_question_names_the_workstream() {
    let f = Fixture::new();
    let id = f.workstream("w", None).await;
    assert_eq!(
        order_of(&f.tree().await, &id),
        Some(Order::clarification("What is this Workstream for?"))
    );
}

#[tokio::test]
async fn a25_a_stored_order_keeps_its_text() {
    let f = Fixture::new();
    let id = f.workstream("w", None).await;
    let stored = Order::clarification("What is this Room for?");
    rusqlite::Connection::open(f.dir.path().join("agents.db"))
        .unwrap()
        .execute(
            "UPDATE nodes SET kind = 'room', work = ? WHERE id = ?",
            rusqlite::params![serde_json::to_string(&stored).unwrap(), id],
        )
        .unwrap();
    let tree = f.reopen().tree().await;
    assert_eq!(order_of(&tree, &id), Some(stored));
}
