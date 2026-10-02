//! Todos: items with blockers, in SQLite. Owner: todos Builder.

mod cycle;
mod store;

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Mutex;

use async_trait::async_trait;
use contracts::todo::{CreateParams, SetBlockersParams, TodoId, UpdateParams};
use contracts::{EventData, Verb};
use rpc::{Bus, Ctx, Module, OpenError, RpcError, params, reply};
use serde_json::Value;
use store::Store;

pub struct Todos {
    store: Mutex<Store>,
}

impl Todos {
    /// `dir` is the Project's `.roundup/` directory. `bus` is for events no call caused.
    pub fn open(dir: &Path, _bus: Bus) -> Result<Self, OpenError> {
        Ok(Self {
            store: Mutex::new(Store::open(&dir.join("todos.db"))?),
        })
    }

    fn create(&self, ctx: &Ctx, p: CreateParams) -> Result<Value, RpcError> {
        let todo = self.store()?.insert(
            &p.title,
            &p.body.unwrap_or_default(),
            &p.blockers.unwrap_or_default(),
            &ctx.actor,
        )?;
        ctx.touch(Verb::Wrote, &item(todo.id))?;
        ctx.emit(EventData::TodoCreated(todo.clone()));
        reply(&todo)
    }

    fn list(&self) -> Result<Value, RpcError> {
        reply(&self.store()?.list()?)
    }

    fn get(&self, ctx: &Ctx, p: TodoId) -> Result<Value, RpcError> {
        let todo = self.store()?.get(p.id)?;
        ctx.touch(Verb::Read, &item(p.id))?;
        reply(&todo)
    }

    fn update(&self, ctx: &Ctx, p: UpdateParams) -> Result<Value, RpcError> {
        if p.title.is_none() && p.body.is_none() {
            return Err(RpcError::new(
                rpc::code::INVALID_PARAMS,
                "update needs a title or a body",
            ));
        }
        let todo = self
            .store()?
            .update(p.id, p.title.as_deref(), p.body.as_deref())?;
        ctx.touch(Verb::Wrote, &item(p.id))?;
        ctx.emit(EventData::TodoUpdated(todo.clone()));
        reply(&todo)
    }

    fn complete(&self, ctx: &Ctx, p: TodoId) -> Result<Value, RpcError> {
        let store = self.store()?;
        let was_blocked = store.blocked_ids()?;
        let todo = store.complete(p.id)?;
        ctx.touch(Verb::Wrote, &item(p.id))?;
        ctx.emit(EventData::TodoUpdated(todo.clone()));
        emit_unblocked(ctx, &was_blocked, &store.blocked_ids()?);
        reply(&todo)
    }

    fn set_blockers(&self, ctx: &Ctx, p: SetBlockersParams) -> Result<Value, RpcError> {
        let store = self.store()?;
        let was_blocked = store.blocked_ids()?;
        store.get(p.id)?;
        store.require_all(&p.blockers)?;
        if cycle::creates_cycle(&store.edges()?, p.id, &p.blockers) {
            return Err(RpcError::conflict(format!(
                "blockers {:?} would make todo {} block itself",
                p.blockers, p.id
            )));
        }
        let todo = store.set_blockers(p.id, &p.blockers)?;
        ctx.touch(Verb::Wrote, &item(p.id))?;
        ctx.emit(EventData::TodoUpdated(todo.clone()));
        emit_unblocked(ctx, &was_blocked, &store.blocked_ids()?);
        reply(&todo)
    }

    fn delete(&self, ctx: &Ctx, p: TodoId) -> Result<Value, RpcError> {
        let store = self.store()?;
        let mut was_blocked = store.blocked_ids()?;
        store.delete(p.id)?;
        ctx.touch(Verb::Wrote, &item(p.id))?;
        was_blocked.remove(&p.id);
        ctx.emit(EventData::TodoDeleted(p));
        emit_unblocked(ctx, &was_blocked, &store.blocked_ids()?);
        Ok(Value::Null)
    }

    fn store(&self) -> Result<std::sync::MutexGuard<'_, Store>, RpcError> {
        self.store
            .lock()
            .map_err(|_| RpcError::internal("todo store poisoned"))
    }
}

/// Emit `todo.unblocked` for every Todo that was blocked before and is not now.
fn emit_unblocked(ctx: &Ctx, before: &BTreeSet<u32>, after: &BTreeSet<u32>) {
    for id in before.difference(after) {
        ctx.emit(EventData::TodoUnblocked(TodoId { id: *id }));
    }
}

fn item(id: u32) -> String {
    format!("todo:{id}")
}

#[async_trait]
impl Module for Todos {
    fn namespaces(&self) -> &'static [&'static str] {
        &["todo"]
    }

    async fn call(&self, ctx: &Ctx, method: &str, value: Value) -> Result<Value, RpcError> {
        match method {
            "todo.create" => self.create(ctx, params(value)?),
            "todo.list" => self.list(),
            "todo.get" => self.get(ctx, params(value)?),
            "todo.update" => self.update(ctx, params(value)?),
            "todo.complete" => self.complete(ctx, params(value)?),
            "todo.setBlockers" => self.set_blockers(ctx, params(value)?),
            "todo.delete" => self.delete(ctx, params(value)?),
            _ => Err(RpcError::method_not_found(method)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use contracts::{Actor, ActorKind, Event};
    use provenance::Touches;
    use rpc::code;
    use serde_json::json;
    use tokio::sync::broadcast::Receiver;

    use super::*;

    struct Harness {
        todos: Todos,
        ctx: Ctx,
        events: Receiver<Event>,
    }

    impl Harness {
        fn new(dir: &Path) -> Self {
            let bus = Bus::new();
            let events = bus.subscribe();
            let ctx = Ctx {
                actor: Actor::user(),
                bus: bus.clone(),
                touches: Arc::new(Touches::in_memory().unwrap()),
            };
            Self {
                todos: Todos::open(dir, bus).unwrap(),
                ctx,
                events,
            }
        }

        async fn call(&self, method: &str, params: Value) -> Result<Value, RpcError> {
            self.todos.call(&self.ctx, method, params).await
        }

        async fn call_as(
            &self,
            actor: Actor,
            method: &str,
            params: Value,
        ) -> Result<Value, RpcError> {
            let ctx = Ctx {
                actor,
                bus: self.ctx.bus.clone(),
                touches: self.ctx.touches.clone(),
            };
            self.todos.call(&ctx, method, params).await
        }

        /// `"<name> <id>"` for each event, oldest first.
        fn events(&mut self) -> Vec<String> {
            let mut seen = Vec::new();
            while let Ok(event) = self.events.try_recv() {
                let event = serde_json::to_value(&event.data).unwrap();
                seen.push(format!(
                    "{} {}",
                    event["name"].as_str().unwrap(),
                    event["data"]["id"]
                ));
            }
            seen
        }

        /// The `data` of the next event, for asserting on a field beyond `id`.
        fn event_data(&mut self) -> Value {
            let event = self.events.try_recv().unwrap();
            serde_json::to_value(&event.data).unwrap()["data"].clone()
        }

        fn touches(&self, id: u32) -> Vec<(Verb, String)> {
            self.ctx
                .touches
                .history(&item(id))
                .unwrap()
                .into_iter()
                .map(|t| (t.verb, t.actor.id))
                .collect()
        }
    }

    #[tokio::test]
    async fn t1_create_assigns_sequential_ids_emits_and_touches() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = Harness::new(dir.path());
        let first = h.call("todo.create", json!({"title": "a"})).await.unwrap();
        assert_eq!(first["id"], 1);
        assert_eq!(first["done"], false);
        assert_eq!(first["blocked"], false);
        assert_eq!(
            h.call("todo.create", json!({"title": "b"})).await.unwrap()["id"],
            2
        );
        assert_eq!(h.events(), ["todo.created 1", "todo.created 2"]);
        assert_eq!(h.touches(1), [(Verb::Wrote, "you".to_owned())]);
    }

    #[tokio::test]
    async fn t2_list_is_in_id_order_and_get_logs_a_read() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path());
        h.call("todo.create", json!({"title": "a"})).await.unwrap();
        h.call("todo.create", json!({"title": "b"})).await.unwrap();
        let ids: Vec<_> = h
            .call("todo.list", json!({}))
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].clone())
            .collect();
        assert_eq!(ids, [1, 2]);
        assert_eq!(
            h.call("todo.get", json!({"id": 2})).await.unwrap()["title"],
            "b"
        );
        assert_eq!(
            h.touches(2),
            [
                (Verb::Wrote, "you".to_owned()),
                (Verb::Read, "you".to_owned())
            ]
        );
        let err = h.call("todo.get", json!({"id": 9})).await.unwrap_err();
        assert_eq!(err.code, code::NOT_FOUND);
    }

    #[tokio::test]
    async fn t5_everything_survives_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let before = {
            let h = Harness::new(dir.path());
            h.call("todo.create", json!({"title": "a", "body": "x"}))
                .await
                .unwrap();
            h.call("todo.create", json!({"title": "b", "blockers": [1]}))
                .await
                .unwrap();
            h.call("todo.complete", json!({"id": 1})).await.unwrap();
            h.call("todo.list", json!({})).await.unwrap()
        };
        let h = Harness::new(dir.path());
        assert_eq!(h.call("todo.list", json!({})).await.unwrap(), before);
        assert_eq!(before[0]["done"], true);
        assert_eq!(before[1]["blockers"], json!([1]));
        assert_eq!(
            h.call("todo.create", json!({"title": "c"})).await.unwrap()["id"],
            3
        );
    }

    #[tokio::test]
    async fn t7_update_changes_only_the_given_fields() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = Harness::new(dir.path());
        h.call("todo.create", json!({"title": "a", "body": "keep"}))
            .await
            .unwrap();
        h.events();
        let todo = h
            .call("todo.update", json!({"id": 1, "title": "x"}))
            .await
            .unwrap();
        assert_eq!(
            (todo["title"].as_str(), todo["body"].as_str()),
            (Some("x"), Some("keep"))
        );
        assert_eq!(h.events(), ["todo.updated 1"]);
        assert_eq!(
            h.touches(1),
            [
                (Verb::Wrote, "you".to_owned()),
                (Verb::Wrote, "you".to_owned())
            ]
        );
        let err = h.call("todo.update", json!({"id": 1})).await.unwrap_err();
        assert_eq!(err.code, code::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn t3_completing_a_blocker_unblocks_the_todo() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = Harness::new(dir.path());
        h.call("todo.create", json!({"title": "a"})).await.unwrap();
        let two = h
            .call("todo.create", json!({"title": "b", "blockers": [1]}))
            .await
            .unwrap();
        assert_eq!(two["blocked"], true);
        h.events();
        h.call("todo.complete", json!({"id": 1})).await.unwrap();
        assert_eq!(h.events(), ["todo.updated 1", "todo.unblocked 2"]);
        assert_eq!(
            h.call("todo.get", json!({"id": 2})).await.unwrap()["blocked"],
            false
        );
        h.call("todo.complete", json!({"id": 1})).await.unwrap();
        assert_eq!(h.events(), ["todo.updated 1"]);
    }

    #[tokio::test]
    async fn t4_bad_blockers_are_rejected_and_change_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path());
        h.call("todo.create", json!({"title": "a"})).await.unwrap();
        h.call("todo.create", json!({"title": "b", "blockers": [1]}))
            .await
            .unwrap();
        let cycle = h
            .call("todo.setBlockers", json!({"id": 1, "blockers": [2]}))
            .await
            .unwrap_err();
        let own = h
            .call("todo.setBlockers", json!({"id": 1, "blockers": [1]}))
            .await
            .unwrap_err();
        let unknown = h
            .call("todo.setBlockers", json!({"id": 2, "blockers": [9]}))
            .await
            .unwrap_err();
        assert_eq!(
            (cycle.code, own.code, unknown.code),
            (code::CONFLICT, code::CONFLICT, code::NOT_FOUND)
        );
        assert_eq!(
            h.call("todo.get", json!({"id": 1})).await.unwrap()["blockers"],
            json!([])
        );
        assert_eq!(
            h.call("todo.get", json!({"id": 2})).await.unwrap()["blockers"],
            json!([1])
        );
    }

    #[tokio::test]
    async fn t4_set_blockers_replaces_the_whole_list() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path());
        for title in ["a", "b", "c"] {
            h.call("todo.create", json!({"title": title}))
                .await
                .unwrap();
        }
        h.call("todo.setBlockers", json!({"id": 3, "blockers": [1]}))
            .await
            .unwrap();
        let todo = h
            .call("todo.setBlockers", json!({"id": 3, "blockers": [2]}))
            .await
            .unwrap();
        assert_eq!(todo["blockers"], json!([2]));
    }

    #[tokio::test]
    async fn t6_deleting_a_blocker_unblocks_the_todo() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = Harness::new(dir.path());
        h.call("todo.create", json!({"title": "a"})).await.unwrap();
        h.call("todo.create", json!({"title": "b", "blockers": [1]}))
            .await
            .unwrap();
        h.events();
        h.call("todo.delete", json!({"id": 1})).await.unwrap();
        assert_eq!(h.events(), ["todo.deleted 1", "todo.unblocked 2"]);
        let two = h.call("todo.get", json!({"id": 2})).await.unwrap();
        assert_eq!(
            (two["blockers"].clone(), two["blocked"].clone()),
            (json!([]), json!(false))
        );
        assert_eq!(
            h.call("todo.delete", json!({"id": 1}))
                .await
                .unwrap_err()
                .code,
            code::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn t6_deleting_a_blocked_todo_announces_only_its_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = Harness::new(dir.path());
        h.call("todo.create", json!({"title": "a"})).await.unwrap();
        h.call("todo.create", json!({"title": "b", "blockers": [1]}))
            .await
            .unwrap();
        h.events();
        let reply = h.call("todo.delete", json!({"id": 2})).await.unwrap();
        assert_eq!(reply, Value::Null);
        assert_eq!(h.events(), ["todo.deleted 2"]);
    }

    fn agent(id: &str) -> Actor {
        Actor {
            kind: ActorKind::Agent,
            id: id.into(),
            parent: None,
        }
    }

    #[tokio::test]
    async fn t8_creator_is_the_calling_actor_on_create_list_get_and_the_event() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = Harness::new(dir.path());

        let by_user = h.call("todo.create", json!({"title": "a"})).await.unwrap();
        assert_eq!(
            by_user["creator"],
            serde_json::to_value(Actor::user()).unwrap()
        );
        assert_eq!(
            h.event_data()["creator"],
            serde_json::to_value(Actor::user()).unwrap()
        );

        // A `creator` sent in the params is ignored: the caller's own Actor wins.
        let by_agent = h
            .call_as(
                agent("a"),
                "todo.create",
                json!({"title": "b", "creator": Actor::user()}),
            )
            .await
            .unwrap();
        assert_eq!(
            by_agent["creator"],
            serde_json::to_value(agent("a")).unwrap()
        );
        assert_eq!(
            h.event_data()["creator"],
            serde_json::to_value(agent("a")).unwrap()
        );

        let listed = h.call("todo.list", json!({})).await.unwrap();
        assert_eq!(
            listed[1]["creator"],
            serde_json::to_value(agent("a")).unwrap()
        );
        let got = h
            .call("todo.get", json!({"id": by_agent["id"]}))
            .await
            .unwrap();
        assert_eq!(got["creator"], serde_json::to_value(agent("a")).unwrap());
    }

    #[tokio::test]
    async fn t8_update_complete_and_set_blockers_never_change_creator() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path());
        h.call_as(agent("a"), "todo.create", json!({"title": "x"}))
            .await
            .unwrap();
        h.call("todo.create", json!({"title": "y"})).await.unwrap();
        let expected = serde_json::to_value(agent("a")).unwrap();

        let updated = h
            .call("todo.update", json!({"id": 1, "title": "z"}))
            .await
            .unwrap();
        assert_eq!(updated["creator"], expected);

        let completed = h.call("todo.complete", json!({"id": 1})).await.unwrap();
        assert_eq!(completed["creator"], expected);

        let reblocked = h
            .call("todo.setBlockers", json!({"id": 1, "blockers": [2]}))
            .await
            .unwrap();
        assert_eq!(reblocked["creator"], expected);
    }

    #[tokio::test]
    async fn t8_creator_persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        {
            let h = Harness::new(dir.path());
            h.call_as(agent("a"), "todo.create", json!({"title": "x"}))
                .await
                .unwrap();
        }
        let h = Harness::new(dir.path());
        let got = h.call("todo.get", json!({"id": 1})).await.unwrap();
        assert_eq!(got["creator"], serde_json::to_value(agent("a")).unwrap());
    }

    /// A `todos.db` written before `creator` existed, by hand, as T8 asks.
    #[tokio::test]
    async fn t8_a_todo_from_before_this_field_reads_as_the_user() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("todos.db");
        {
            let old = rusqlite::Connection::open(&db_path).unwrap();
            old.execute_batch(
                "CREATE TABLE todos (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    title TEXT NOT NULL,
                    body TEXT NOT NULL,
                    done INTEGER NOT NULL DEFAULT 0,
                    created_at INTEGER NOT NULL
                );
                CREATE TABLE blockers (
                    todo INTEGER NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
                    blocker INTEGER NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
                    PRIMARY KEY (todo, blocker)
                );
                INSERT INTO todos (title, body, created_at) VALUES ('old', '', 0);",
            )
            .unwrap();
        }

        let h = Harness::new(dir.path());
        let got = h.call("todo.get", json!({"id": 1})).await.unwrap();
        assert_eq!(got["creator"], serde_json::to_value(Actor::user()).unwrap());

        let created = h
            .call("todo.create", json!({"title": "new"}))
            .await
            .unwrap();
        assert_eq!(
            created["creator"],
            serde_json::to_value(Actor::user()).unwrap()
        );
    }
}
