use std::sync::Arc;

use contracts::{Actor, ActorKind, EventData, Verb};
use pads::Pads;
use provenance::Touches;
use rpc::{Bus, Ctx, Module, code};
use serde_json::{Value, json};
use tempfile::TempDir;

fn agent(id: &str) -> Actor {
    Actor {
        kind: ActorKind::Agent,
        id: id.into(),
        parent: None,
    }
}

struct Rig {
    pads: Pads,
    bus: Bus,
    touches: Arc<Touches>,
    dir: TempDir,
}

impl Rig {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let bus = Bus::new();
        Self {
            pads: Pads::open(dir.path(), bus.clone()).unwrap(),
            bus,
            touches: Arc::new(Touches::in_memory().unwrap()),
            dir,
        }
    }

    async fn call(
        &self,
        actor: &Actor,
        method: &str,
        params: Value,
    ) -> Result<Value, rpc::RpcError> {
        let ctx = Ctx {
            actor: actor.clone(),
            bus: self.bus.clone(),
            touches: Arc::clone(&self.touches),
        };
        self.pads.call(&ctx, method, params).await
    }

    async fn ok(&self, actor: &Actor, method: &str, params: Value) -> Value {
        self.call(actor, method, params).await.unwrap()
    }

    async fn fail(&self, actor: &Actor, method: &str, params: Value) -> rpc::RpcError {
        self.call(actor, method, params).await.unwrap_err()
    }
}

#[tokio::test]
async fn p1_create_makes_the_caller_owner_and_logs_a_write() {
    let rig = Rig::new();
    let mut events = rig.bus.subscribe();
    let a = agent("a");

    let pad = rig
        .ok(&a, "pad.create", json!({"name": "notes", "text": "hi"}))
        .await;

    assert_eq!(pad["owner"]["id"], "a");
    assert_eq!(pad["text"], "hi");
    let duplicate = rig.fail(&a, "pad.create", json!({"name": "notes"})).await;
    assert_eq!(duplicate.code, code::CONFLICT);
    let EventData::PadChanged(changed) = events.try_recv().unwrap().data else {
        panic!("expected pad.changed");
    };
    assert_eq!(changed.name, "notes");
    let history = rig.touches.history("pad:notes").unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].verb, Verb::Wrote);
}

#[tokio::test]
async fn p2_owner_rewrites_and_others_only_append() {
    let rig = Rig::new();
    let (a, b) = (agent("a"), agent("b"));
    rig.ok(&a, "pad.create", json!({"name": "notes", "text": "one"}))
        .await;

    rig.ok(&a, "pad.write", json!({"name": "notes", "text": "two"}))
        .await;
    let mut events = rig.bus.subscribe();
    let denied = rig
        .fail(&b, "pad.write", json!({"name": "notes", "text": "x"}))
        .await;
    assert_eq!(denied.code, code::FORBIDDEN);
    assert!(events.try_recv().is_err());
    rig.ok(&b, "pad.append", json!({"name": "notes", "text": "+b"}))
        .await;

    let pad = rig.ok(&a, "pad.read", json!({"name": "notes"})).await;
    assert_eq!(pad["text"], "two+b");
    assert!(matches!(
        events.try_recv().unwrap().data,
        EventData::PadChanged(_)
    ));
    let by_b: Vec<_> = rig.touches.touched("b").unwrap();
    assert_eq!(by_b.len(), 1);
    assert_eq!(by_b[0].verb, Verb::Wrote);
}

#[tokio::test]
async fn p3_list_is_ordered_and_read_is_logged() {
    let rig = Rig::new();
    let a = agent("a");
    for name in ["c", "a", "b"] {
        rig.ok(&a, "pad.create", json!({"name": name, "text": name}))
            .await;
    }

    let listed = rig.ok(&a, "pad.list", Value::Null).await;
    let names: Vec<_> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["a", "b", "c"]);
    let read = rig.ok(&a, "pad.read", json!({"name": "b"})).await;
    assert_eq!(
        (read["name"].as_str(), read["text"].as_str()),
        (Some("b"), Some("b"))
    );
    rig.ok(&a, "pad.read", json!({"name": "a"})).await;
    let verbs: Vec<_> = rig
        .touches
        .history("pad:a")
        .unwrap()
        .iter()
        .map(|t| t.verb)
        .collect();
    assert_eq!(verbs, [Verb::Wrote, Verb::Read]);
    let missing = rig.fail(&a, "pad.read", json!({"name": "zzz"})).await;
    assert_eq!(missing.code, code::NOT_FOUND);
}

#[tokio::test]
async fn p8_bad_names_are_invalid_params_for_every_method() {
    let rig = Rig::new();
    let a = agent("a");
    for method in [
        "pad.create",
        "pad.read",
        "pad.write",
        "pad.append",
        "pad.delete",
        "pad.setOwner",
        "pad.export",
    ] {
        for name in ["", "a/b", "a\\b", "..", ".hidden", "a..b", "a\0b"] {
            let err = rig
                .fail(
                    &a,
                    method,
                    json!({"name": name, "text": "x", "owner": a, "path": "x.md"}),
                )
                .await;
            assert_eq!(err.code, code::INVALID_PARAMS, "{method} {name:?}");
        }
    }
}

fn user() -> Actor {
    Actor::user()
}

#[tokio::test]
async fn p4_user_flips_ownership_and_owner_may_hand_over() {
    let rig = Rig::new();
    let (a, b) = (agent("a"), agent("b"));
    rig.call(&a, "pad.create", json!({"name": "notes"}))
        .await
        .unwrap();

    let denied = rig
        .call(&b, "pad.setOwner", json!({"name": "notes", "owner": b}))
        .await
        .unwrap_err();
    assert_eq!(denied.code, code::FORBIDDEN);
    let flipped = rig
        .call(
            &user(),
            "pad.setOwner",
            json!({"name": "notes", "owner": user()}),
        )
        .await
        .unwrap();
    assert_eq!(flipped["owner"]["kind"], "user");

    let locked_out = rig
        .call(&a, "pad.write", json!({"name": "notes", "text": "x"}))
        .await
        .unwrap_err();
    assert_eq!(locked_out.code, code::FORBIDDEN);
    rig.call(
        &user(),
        "pad.write",
        json!({"name": "notes", "text": "mine"}),
    )
    .await
    .unwrap();
    let handed = rig
        .call(
            &user(),
            "pad.setOwner",
            json!({"name": "notes", "owner": b}),
        )
        .await
        .unwrap();
    assert_eq!(handed["owner"]["id"], "b");
}

#[tokio::test]
async fn p6_export_writes_one_file_and_never_creates_directories() {
    let rig = Rig::new();
    let a = agent("a");
    rig.call(&a, "pad.create", json!({"name": "notes", "text": "hello"}))
        .await
        .unwrap();
    let out = TempDir::new().unwrap();

    let target = out.path().join("notes.md");
    rig.call(&a, "pad.export", json!({"name": "notes", "path": target}))
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "hello");
    rig.ok(&a, "pad.setStorage", json!({"files": true})).await;
    let in_files_mode = out.path().join("files-mode.md");
    rig.ok(
        &a,
        "pad.export",
        json!({"name": "notes", "path": in_files_mode}),
    )
    .await;
    assert_eq!(std::fs::read_to_string(&in_files_mode).unwrap(), "hello");

    let missing_dir = out.path().join("nope").join("notes.md");
    let err = rig
        .call(
            &a,
            "pad.export",
            json!({"name": "notes", "path": missing_dir}),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, code::INVALID_PARAMS);
    assert!(!out.path().join("nope").exists());
}

#[tokio::test]
async fn p7_pads_survive_reopening_the_directory() {
    let dir = TempDir::new().unwrap();
    let a = agent("a");
    let touches = Arc::new(Touches::in_memory().unwrap());
    let call = |pads: Pads, method: &'static str, params: Value| {
        let ctx = Ctx {
            actor: a.clone(),
            bus: Bus::new(),
            touches: Arc::clone(&touches),
        };
        async move { pads.call(&ctx, method, params).await.unwrap() }
    };
    let first = Pads::open(dir.path(), Bus::new()).unwrap();
    call(
        first,
        "pad.create",
        json!({"name": "notes", "text": "kept"}),
    )
    .await;

    let reopened = Pads::open(dir.path(), Bus::new()).unwrap();
    let pad = call(reopened, "pad.read", json!({"name": "notes"})).await;

    assert_eq!(pad["text"], "kept");
    assert_eq!(pad["owner"]["id"], "a");
}

#[tokio::test]
async fn p9_owner_or_user_deletes_and_others_are_forbidden() {
    let rig = Rig::new();
    let (a, b) = (agent("a"), agent("b"));
    rig.ok(&a, "pad.create", json!({"name": "notes"})).await;
    rig.ok(&a, "pad.create", json!({"name": "plan"})).await;

    let denied = rig.fail(&b, "pad.delete", json!({"name": "notes"})).await;
    assert_eq!(denied.code, code::FORBIDDEN);
    let mut events = rig.bus.subscribe();
    rig.ok(&a, "pad.delete", json!({"name": "notes"})).await;
    rig.ok(&user(), "pad.delete", json!({"name": "plan"})).await;

    let gone = rig.fail(&a, "pad.read", json!({"name": "notes"})).await;
    assert_eq!(gone.code, code::NOT_FOUND);
    let EventData::PadChanged(changed) = events.try_recv().unwrap().data else {
        panic!("expected pad.changed");
    };
    assert_eq!(changed.name, "notes");
    let verbs: Vec<_> = rig
        .touches
        .history("pad:notes")
        .unwrap()
        .iter()
        .map(|t| t.verb)
        .collect();
    assert_eq!(verbs, [Verb::Wrote, Verb::Wrote]);
}

#[tokio::test]
async fn p5_files_mirror_pads_and_edits_import_when_flipped_back() {
    let rig = Rig::new();
    let a = agent("a");
    rig.call(&a, "pad.create", json!({"name": "notes", "text": "one"}))
        .await
        .unwrap();
    rig.call(&a, "pad.create", json!({"name": "plan", "text": "p"}))
        .await
        .unwrap();
    let notes = rig.dir.path().join("pads").join("notes.md");

    rig.call(&a, "pad.setStorage", json!({"files": true}))
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(&notes).unwrap(), "one");
    assert_eq!(
        std::fs::read_to_string(rig.dir.path().join("pads/plan.md")).unwrap(),
        "p"
    );

    rig.call(&a, "pad.write", json!({"name": "notes", "text": "two"}))
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(&notes).unwrap(), "two");
    rig.call(
        &agent("b"),
        "pad.append",
        json!({"name": "notes", "text": "+b"}),
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read_to_string(&notes).unwrap(), "two+b");

    let reopened = Pads::open(rig.dir.path(), Bus::new()).unwrap();
    let ctx = Ctx {
        actor: a.clone(),
        bus: Bus::new(),
        touches: Arc::clone(&rig.touches),
    };
    reopened
        .call(&ctx, "pad.append", json!({"name": "notes", "text": "!"}))
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(&notes).unwrap(), "two+b!");

    std::fs::write(&notes, "edited on disk").unwrap();
    reopened
        .call(&ctx, "pad.setStorage", json!({"files": false}))
        .await
        .unwrap();

    let pad = reopened
        .call(&ctx, "pad.read", json!({"name": "notes"}))
        .await
        .unwrap();
    assert_eq!(pad["text"], "edited on disk");
    assert_eq!(pad["owner"]["id"], "a");
}

#[tokio::test]
async fn p9_delete_removes_the_file_when_file_backed() {
    let rig = Rig::new();
    let a = agent("a");
    rig.ok(&a, "pad.setStorage", json!({"files": true})).await;
    rig.ok(&a, "pad.create", json!({"name": "notes"})).await;
    let file = rig.dir.path().join("pads/notes.md");
    assert!(file.exists());

    rig.ok(&a, "pad.delete", json!({"name": "notes"})).await;

    assert!(!file.exists());
}

#[tokio::test]
async fn p4_owner_may_hand_over_its_own_pad() {
    let rig = Rig::new();
    let (a, b) = (agent("a"), agent("b"));
    rig.ok(&a, "pad.create", json!({"name": "notes"})).await;

    let handed = rig
        .ok(&a, "pad.setOwner", json!({"name": "notes", "owner": b}))
        .await;

    assert_eq!(handed["owner"]["id"], "b");
    let again = rig
        .fail(&a, "pad.setOwner", json!({"name": "notes", "owner": a}))
        .await;
    assert_eq!(again.code, code::FORBIDDEN);
}

#[tokio::test]
async fn p1_names_are_unique_ignoring_case() {
    let rig = Rig::new();
    let a = agent("a");
    rig.ok(&a, "pad.create", json!({"name": "notes", "text": "t"}))
        .await;

    let clash = rig.fail(&a, "pad.create", json!({"name": "Notes"})).await;

    assert_eq!(clash.code, code::CONFLICT);
    let found = rig.ok(&a, "pad.read", json!({"name": "NOTES"})).await;
    assert_eq!(found["name"], "notes");
}

#[tokio::test]
async fn p5_a_failed_flip_back_applies_nothing_and_can_be_retried() {
    let rig = Rig::new();
    let a = agent("a");
    rig.ok(&a, "pad.create", json!({"name": "notes", "text": "n"}))
        .await;
    rig.ok(&a, "pad.create", json!({"name": "plan", "text": "p"}))
        .await;
    rig.ok(&a, "pad.setStorage", json!({"files": true})).await;
    let files = rig.dir.path().join("pads");
    std::fs::write(files.join("notes.md"), "edited").unwrap();
    std::fs::write(files.join("plan.md"), [0xff, 0xfe]).unwrap();

    let err = rig
        .fail(&a, "pad.setStorage", json!({"files": false}))
        .await;

    assert_eq!(err.code, code::INTERNAL);
    let notes = rig.ok(&a, "pad.read", json!({"name": "notes"})).await;
    assert_eq!(notes["text"], "n");
    std::fs::write(files.join("plan.md"), "fixed").unwrap();
    rig.ok(&a, "pad.setStorage", json!({"files": false})).await;
    let notes = rig.ok(&a, "pad.read", json!({"name": "notes"})).await;
    assert_eq!(notes["text"], "edited");
}

#[tokio::test]
async fn p5_a_failed_file_write_changes_nothing_and_emits_nothing() {
    let rig = Rig::new();
    let a = agent("a");
    rig.ok(&a, "pad.create", json!({"name": "notes", "text": "old"}))
        .await;
    rig.ok(&a, "pad.setStorage", json!({"files": true})).await;
    let blocked = rig.dir.path().join("pads");
    std::fs::remove_file(blocked.join("notes.md")).unwrap();
    std::fs::create_dir(blocked.join("notes.md")).unwrap();
    std::fs::create_dir(blocked.join("other.md")).unwrap();
    let mut events = rig.bus.subscribe();
    let touched = rig.touches.touched("a").unwrap().len();

    rig.fail(&a, "pad.write", json!({"name": "notes", "text": "new"}))
        .await;
    rig.fail(&a, "pad.append", json!({"name": "notes", "text": "+"}))
        .await;
    rig.fail(&a, "pad.create", json!({"name": "other"})).await;

    let pad = rig.ok(&a, "pad.read", json!({"name": "notes"})).await;
    assert_eq!(pad["text"], "old");
    assert_eq!(
        rig.ok(&a, "pad.list", Value::Null)
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(events.try_recv().is_err());
    assert_eq!(
        rig.touches.touched("a").unwrap().len(),
        touched + 1,
        "only the read"
    );
}

#[tokio::test]
async fn p9_a_failed_file_removal_deletes_nothing() {
    let rig = Rig::new();
    let a = agent("a");
    rig.ok(&a, "pad.create", json!({"name": "notes", "text": "keep"}))
        .await;
    rig.ok(&a, "pad.setStorage", json!({"files": true})).await;
    let file = rig.dir.path().join("pads/notes.md");
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    std::fs::write(file.join("inner"), "x").unwrap();
    let mut events = rig.bus.subscribe();

    rig.fail(&a, "pad.delete", json!({"name": "notes"})).await;

    let pad = rig.ok(&a, "pad.read", json!({"name": "notes"})).await;
    assert_eq!(pad["text"], "keep");
    assert!(events.try_recv().is_err());
    let verbs: Vec<_> = rig
        .touches
        .history("pad:notes")
        .unwrap()
        .iter()
        .map(|t| t.verb)
        .collect();
    assert_eq!(verbs, [Verb::Wrote, Verb::Read]);
}

#[tokio::test]
async fn p5_a_write_is_atomic_so_hard_links_keep_the_old_text() {
    let rig = Rig::new();
    let a = agent("a");
    rig.ok(&a, "pad.create", json!({"name": "notes", "text": "old"}))
        .await;
    rig.ok(&a, "pad.setStorage", json!({"files": true})).await;
    let files = rig.dir.path().join("pads");
    std::fs::hard_link(files.join("notes.md"), files.join("link.md")).unwrap();

    rig.ok(&a, "pad.write", json!({"name": "notes", "text": "new"}))
        .await;

    assert_eq!(
        std::fs::read_to_string(files.join("notes.md")).unwrap(),
        "new"
    );
    assert_eq!(
        std::fs::read_to_string(files.join("link.md")).unwrap(),
        "old"
    );
}

#[tokio::test]
async fn p5_a_case_clash_in_file_mode_leaves_the_first_pad_and_its_file() {
    let rig = Rig::new();
    let a = agent("a");
    rig.ok(&a, "pad.setStorage", json!({"files": true})).await;
    rig.ok(&a, "pad.create", json!({"name": "notes", "text": "first"}))
        .await;

    let clash = rig
        .fail(&a, "pad.create", json!({"name": "Notes", "text": "second"}))
        .await;

    assert_eq!(clash.code, code::CONFLICT);
    let files = rig.dir.path().join("pads");
    assert_eq!(
        std::fs::read_to_string(files.join("notes.md")).unwrap(),
        "first"
    );
    assert_eq!(std::fs::read_dir(&files).unwrap().count(), 1);
}

#[tokio::test]
async fn p5_a_pair_that_once_collided_shares_one_file_and_survives_the_flip() {
    let rig = Rig::new();
    let a = agent("a");
    rig.ok(&a, "pad.create", json!({"name": "ΑΣ", "text": "one"}))
        .await;
    rig.fail(&a, "pad.create", json!({"name": "ασ", "text": "two"}))
        .await;

    rig.ok(&a, "pad.setStorage", json!({"files": true})).await;
    let files = rig.dir.path().join("pads");
    assert_eq!(std::fs::read_dir(&files).unwrap().count(), 1);
    rig.ok(&a, "pad.setStorage", json!({"files": false})).await;

    let pad = rig.ok(&a, "pad.read", json!({"name": "ασ"})).await;
    assert_eq!(
        (pad["name"].as_str(), pad["text"].as_str()),
        (Some("ΑΣ"), Some("one"))
    );
}

#[tokio::test]
async fn p8_a_stored_pad_with_a_bad_name_fails_the_flip_as_invalid_params() {
    let rig = Rig::new();
    let a = agent("a");
    rig.ok(&a, "pad.setStorage", json!({"files": true})).await;
    let nul = rig.fail(&a, "pad.create", json!({"name": "a\0b"})).await;
    assert_eq!(nul.code, code::INVALID_PARAMS);
    rig.ok(&a, "pad.setStorage", json!({"files": false})).await;
    rusqlite::Connection::open(rig.dir.path().join("pads.db"))
        .unwrap()
        .execute(
            "INSERT INTO pads (key, name, owner, text, updated_at) VALUES ('k', ?1, ?2, '', 0)",
            ["a\0b".to_string(), serde_json::to_string(&a).unwrap()],
        )
        .unwrap();

    let err = rig.fail(&a, "pad.setStorage", json!({"files": true})).await;

    assert_eq!(err.code, code::INVALID_PARAMS);
    assert!(err.message.contains("a\\0b"), "{}", err.message);
}

#[tokio::test]
async fn p5_a_name_the_file_system_refuses_is_invalid_params() {
    let rig = Rig::new();
    let a = agent("a");
    rig.ok(&a, "pad.setStorage", json!({"files": true})).await;

    let err = rig
        .fail(&a, "pad.create", json!({"name": "x".repeat(300)}))
        .await;

    assert_eq!(err.code, code::INVALID_PARAMS);
}

#[tokio::test]
async fn p1_names_that_fold_or_normalize_together_conflict() {
    let rig = Rig::new();
    let a = agent("a");
    for (first, second) in [
        ("ΑΣ", "ασ"),
        ("straße", "STRASSE"),
        ("caf\u{e9}", "cafe\u{301}"),
    ] {
        rig.ok(&a, "pad.create", json!({"name": first})).await;
        let clash = rig.fail(&a, "pad.create", json!({"name": second})).await;
        assert_eq!(clash.code, code::CONFLICT, "{first} vs {second}");
    }
}

#[tokio::test]
async fn p1_every_method_resolves_other_spellings_to_the_same_pad() {
    let rig = Rig::new();
    let (a, b) = (agent("a"), agent("b"));
    let out = TempDir::new().unwrap();
    rig.ok(&a, "pad.create", json!({"name": "Notes", "text": "x"}))
        .await;

    rig.ok(&a, "pad.write", json!({"name": "nOTES", "text": "y"}))
        .await;
    rig.ok(&b, "pad.append", json!({"name": "NOTES", "text": "z"}))
        .await;
    let read = rig.ok(&a, "pad.read", json!({"name": "notes"})).await;
    assert_eq!(
        (read["name"].as_str(), read["text"].as_str()),
        (Some("Notes"), Some("yz"))
    );
    rig.ok(&a, "pad.setOwner", json!({"name": "nOtEs", "owner": b}))
        .await;
    let path = out.path().join("n.md");
    rig.ok(&b, "pad.export", json!({"name": "NOTES", "path": path}))
        .await;
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "yz");
    rig.ok(&b, "pad.delete", json!({"name": "noTES"})).await;

    let gone = rig.fail(&a, "pad.read", json!({"name": "Notes"})).await;
    assert_eq!(gone.code, code::NOT_FOUND);
    assert!(
        rig.ok(&a, "pad.list", Value::Null)
            .await
            .as_array()
            .unwrap()
            .is_empty()
    );
}
