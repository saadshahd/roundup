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
        let was_blocked = store.blocked_ids()?;
        store.delete(p.id)?;
        ctx.touch(Verb::Wrote, &item(p.id))?;
        ctx.emit(EventData::TodoDeleted(p.clone()));
        emit_unblocked(ctx, &was_blocked, &store.blocked_ids()?);
        reply(&p)
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

    use contracts::{Actor, Event};
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

        fn events(&mut self) -> Vec<String> {
            let mut names = Vec::new();
            while let Ok(event) = self.events.try_recv() {
                names.push(
                    serde_json::to_value(&event.data).unwrap()["name"]
                        .as_str()
                        .unwrap()
                        .to_owned(),
                );
            }
            names
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
        assert_eq!(h.events(), ["todo.created", "todo.created"]);
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
        assert_eq!(h.events(), ["todo.updated"]);
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
        assert_eq!(h.events(), ["todo.updated", "todo.unblocked"]);
        assert_eq!(
            h.call("todo.get", json!({"id": 2})).await.unwrap()["blocked"],
            false
        );
        h.call("todo.complete", json!({"id": 1})).await.unwrap();
        assert_eq!(h.events(), ["todo.updated"]);
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
        assert_eq!(h.events(), ["todo.deleted", "todo.unblocked"]);
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
}
