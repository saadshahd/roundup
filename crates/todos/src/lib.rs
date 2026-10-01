//! Todos: items with blockers, in SQLite. Owner: todos Builder.

mod store;

use std::path::Path;
use std::sync::Mutex;

use async_trait::async_trait;
use contracts::todo::{CreateParams, TodoId, UpdateParams};
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

    fn store(&self) -> Result<std::sync::MutexGuard<'_, Store>, RpcError> {
        self.store
            .lock()
            .map_err(|_| RpcError::internal("todo store poisoned"))
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
            h.call("todo.list", json!({})).await.unwrap()
        };
        let h = Harness::new(dir.path());
        assert_eq!(h.call("todo.list", json!({})).await.unwrap(), before);
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
}
