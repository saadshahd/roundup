//! Messages between Actors, and the Routes that decide how they are delivered. Owner: messages
//! Builder. This slice stores Messages and Routes and answers the user's and an Actor's calls;
//! it never types into a Terminal, which is a later slice's job (H11, `Agents::prompt`).

mod store;

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use contracts::agent::{NodeKind, RailNode};
use contracts::message::{
    Delivery, Held, ListParams, Message, MessageId, MessageStatus, Route, SendParams,
    SetRouteParams,
};
use contracts::{Actor, ActorKind, EventData, Kind, Verb};
use rpc::{Bus, Ctx, Module, OpenError, RpcError, code, params, reply};
use serde_json::Value;
use store::Store;

/// B1's bound on a body's length.
const MAX_BODY_BYTES: usize = 8192;
/// B1's bound on `pending` or `held` Messages a single receiver may hold at once.
const OPEN_BOUND: u32 = 32;

pub struct Messages {
    store: Mutex<Store>,
    /// Answers `rail.tree`, the only thing `message.send` needs to know about a receiver: whether
    /// it names an Agent (or a Meta-agent) at all, and that Agent's Kind.
    agents: Arc<dyn Module>,
}

impl Messages {
    /// `dir` is the Project's `.roundup/` directory. `bus` is for events no call caused.
    pub fn open(dir: &Path, _bus: Bus, agents: Arc<dyn Module>) -> Result<Self, OpenError> {
        Ok(Self {
            store: Mutex::new(Store::open(&dir.join("messages.db"))?),
            agents,
        })
    }

    async fn send(&self, ctx: &Ctx, p: SendParams) -> Result<Value, RpcError> {
        if p.to == ctx.actor.id {
            return Err(RpcError::new(
                code::INVALID_PARAMS,
                "a Message cannot be sent to its own sender",
            ));
        }
        let receiver_status = self.resolve_receiver(ctx, &p.to).await?;
        if p.body.is_empty() || p.body.len() > MAX_BODY_BYTES {
            return Err(RpcError::new(
                code::INVALID_PARAMS,
                format!("body must be 1 to {MAX_BODY_BYTES} bytes"),
            ));
        }
        if let Some(status) = &receiver_status
            && matches!(status.kind, Kind::Done | Kind::Error)
        {
            return Err(RpcError::conflict(format!(
                "{} cannot receive a Message: its Kind is {:?}",
                p.to, status.kind
            )));
        }
        // The reply-to check, the bound check and the insert run while one lock is held, so a
        // second sender racing for the same receiver's last open slot cannot pass its own check
        // before this one's insert lands (B1).
        let message = {
            let store = self.store()?;
            if let Some(reply_to) = p.reply_to
                && store.get(reply_to)?.is_none()
            {
                return Err(RpcError::new(
                    code::INVALID_PARAMS,
                    format!("replyTo names no Message: {reply_to}"),
                ));
            }
            if store.count_open(&p.to)? >= OPEN_BOUND {
                return Err(RpcError::conflict(format!(
                    "{} already has {OPEN_BOUND} pending or held Messages",
                    p.to
                )));
            }
            // B10: a Message to the user is delivered at once, whatever Routes exist; the
            // user has no `idle` for B2's pending-until-idle rule to wait on.
            let (status, reason) = if p.to == Actor::user().id {
                (MessageStatus::Delivered, None)
            } else {
                match store
                    .get_route(&ctx.actor.id, &p.to)?
                    .unwrap_or(Delivery::Auto)
                {
                    Delivery::Auto => (MessageStatus::Pending, None),
                    Delivery::AskFirst => (MessageStatus::Held, Some(Held::AskFirst)),
                    Delivery::Drop => (MessageStatus::Dropped, None),
                }
            };
            store.insert(
                &ctx.actor,
                &p.to,
                p.kind,
                &p.body,
                p.reply_to,
                status,
                reason,
                now_ms(),
            )?
        };
        ctx.touch(Verb::Wrote, &item(message.id))?;
        ctx.emit(EventData::MessageSent(message.clone()));
        match message.status {
            MessageStatus::Held => ctx.emit(EventData::MessageHeld(message.clone())),
            MessageStatus::Delivered => ctx.emit(EventData::MessageDelivered(message.clone())),
            MessageStatus::Dropped => ctx.emit(EventData::MessageDropped(message.clone())),
            MessageStatus::Pending => {}
        }
        reply(&message)
    }

    fn get(&self, ctx: &Ctx, p: MessageId) -> Result<Value, RpcError> {
        let message = self.get_or_not_found(p.id)?;
        require_readable(ctx, &message)?;
        ctx.touch(Verb::Read, &item(p.id))?;
        reply(&message)
    }

    fn list(&self, ctx: &Ctx, p: ListParams) -> Result<Value, RpcError> {
        let mut messages = self.store()?.list()?;
        if let Some(to) = &p.to {
            messages.retain(|message| &message.to == to);
        }
        if let Some(status) = p.status {
            messages.retain(|message| message.status == status);
        }
        if ctx.actor.kind != ActorKind::User {
            messages
                .retain(|message| message.from.id == ctx.actor.id || message.to == ctx.actor.id);
        }
        reply(&messages)
    }

    /// B3: moves a `held` Message to `pending`; B2 (typing it in) is a later slice.
    fn deliver(&self, ctx: &Ctx, p: MessageId) -> Result<Value, RpcError> {
        require_user(ctx, "deliver a held Message")?;
        self.release_held(p.id, MessageStatus::Pending, None)?;
        ctx.touch(Verb::Wrote, &item(p.id))?;
        reply(&self.get_or_not_found(p.id)?)
    }

    fn drop_message(&self, ctx: &Ctx, p: MessageId) -> Result<Value, RpcError> {
        require_user(ctx, "drop a held Message")?;
        self.release_held(p.id, MessageStatus::Dropped, None)?;
        ctx.touch(Verb::Wrote, &item(p.id))?;
        let dropped = self.get_or_not_found(p.id)?;
        ctx.emit(EventData::MessageDropped(dropped.clone()));
        reply(&dropped)
    }

    fn set_route(&self, ctx: &Ctx, p: SetRouteParams) -> Result<Value, RpcError> {
        require_user(ctx, "set a Route")?;
        if p.to == Actor::user().id {
            return Err(RpcError::new(
                code::INVALID_PARAMS,
                "a Route cannot deliver to the user",
            ));
        }
        if p.from == p.to {
            return Err(RpcError::new(
                code::INVALID_PARAMS,
                "a Route's from and to must differ",
            ));
        }
        self.store()?.set_route(&p.from, &p.to, p.delivery)?;
        let route = Route {
            from: p.from,
            to: p.to,
            delivery: p.delivery,
        };
        ctx.emit(EventData::RouteChanged(route.clone()));
        reply(&route)
    }

    fn list_routes(&self) -> Result<Value, RpcError> {
        reply(&self.store()?.list_routes()?)
    }

    /// `to`'s Status, when it names an Agent or a Meta-agent; `None` when it names the user.
    /// `NOT_FOUND` when it names no Actor; `INVALID_PARAMS` when it names a Terminal or a plain
    /// Group, neither of which can receive a Message.
    async fn resolve_receiver(
        &self,
        ctx: &Ctx,
        to: &str,
    ) -> Result<Option<contracts::Status>, RpcError> {
        if to == Actor::user().id {
            return Ok(None);
        }
        let query_ctx = Ctx {
            actor: Actor::daemon(),
            bus: ctx.bus.clone(),
            touches: Arc::clone(&ctx.touches),
        };
        let tree = self
            .agents
            .call(&query_ctx, "rail.tree", Value::Null)
            .await?;
        let nodes: Vec<RailNode> = serde_json::from_value(tree).map_err(RpcError::internal)?;
        let node = nodes
            .into_iter()
            .find(|node| node.id == to)
            .ok_or_else(|| RpcError::not_found(format!("actor {to}")))?;
        if node.kind == NodeKind::Terminal || (node.kind == NodeKind::Group && !node.meta) {
            return Err(RpcError::new(
                code::INVALID_PARAMS,
                format!("{to} cannot receive a Message: it is a {:?}", node.kind),
            ));
        }
        Ok(node.status)
    }

    /// Moves a `held` Message to `status`, atomically with the check that it is still `held`
    /// (B3): two concurrent `deliver`/`drop` calls on the same Message can never both apply.
    /// `NOT_FOUND` when no such Message exists, `CONFLICT` when it exists but is not `held`.
    fn release_held(
        &self,
        id: u32,
        status: MessageStatus,
        reason: Option<Held>,
    ) -> Result<(), RpcError> {
        if self.store()?.release_held(id, status, reason)? {
            return Ok(());
        }
        self.get_or_not_found(id)?;
        Err(RpcError::conflict(format!("message {id} is not held")))
    }

    fn get_or_not_found(&self, id: u32) -> Result<Message, RpcError> {
        self.store()?
            .get(id)?
            .ok_or_else(|| RpcError::not_found(format!("message {id}")))
    }

    fn store(&self) -> Result<MutexGuard<'_, Store>, RpcError> {
        self.store
            .lock()
            .map_err(|_| RpcError::internal("message store poisoned"))
    }
}

fn require_user(ctx: &Ctx, action: &str) -> Result<(), RpcError> {
    if ctx.actor.kind == ActorKind::User {
        return Ok(());
    }
    Err(RpcError::forbidden(format!("only the user may {action}")))
}

/// B11: the user may read any Message; anyone else only one they sent or received.
fn require_readable(ctx: &Ctx, message: &Message) -> Result<(), RpcError> {
    if ctx.actor.kind == ActorKind::User
        || message.from.id == ctx.actor.id
        || message.to == ctx.actor.id
    {
        return Ok(());
    }
    Err(RpcError::forbidden(format!(
        "{} may not read message {}",
        ctx.actor.id, message.id
    )))
}

fn item(id: u32) -> String {
    format!("message:{id}")
}

fn now_ms() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(i64::MAX)
}

#[async_trait]
impl Module for Messages {
    fn namespaces(&self) -> &'static [&'static str] {
        &["message", "route"]
    }

    async fn call(&self, ctx: &Ctx, method: &str, value: Value) -> Result<Value, RpcError> {
        match method {
            "message.send" => self.send(ctx, params(value)?).await,
            "message.get" => self.get(ctx, params(value)?),
            "message.list" => self.list(ctx, params(value)?),
            "message.deliver" => self.deliver(ctx, params(value)?),
            "message.drop" => self.drop_message(ctx, params(value)?),
            "route.set" => self.set_route(ctx, params(value)?),
            "route.list" => self.list_routes(),
            _ => Err(RpcError::method_not_found(method)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use contracts::agent::{NodeKind, RailNode};
    use contracts::{Actor, ActorKind, Event, Kind, Status};
    use provenance::Touches;
    use rpc::code;
    use serde_json::json;
    use tokio::sync::broadcast::Receiver;

    use super::*;

    /// Stands in for the `agents` module's `rail.tree`: the only Rail fact `message.send` needs,
    /// without a real Agent, Terminal or Launcher.
    struct FakeRail(StdMutex<Vec<RailNode>>);

    impl FakeRail {
        fn new(nodes: Vec<RailNode>) -> Arc<Self> {
            Arc::new(Self(StdMutex::new(nodes)))
        }
    }

    #[async_trait]
    impl Module for FakeRail {
        fn namespaces(&self) -> &'static [&'static str] {
            &["agent", "rail"]
        }

        async fn call(&self, _ctx: &Ctx, method: &str, _params: Value) -> Result<Value, RpcError> {
            match method {
                "rail.tree" => Ok(serde_json::to_value(&*self.0.lock().unwrap()).unwrap()),
                _ => Err(RpcError::method_not_found(method)),
            }
        }
    }

    fn node(id: &str, kind: NodeKind, meta: bool, status: Option<Status>) -> RailNode {
        RailNode {
            id: id.into(),
            kind,
            name: id.into(),
            parent: None,
            order: 0,
            status,
            meta,
            terminal_id: None,
        }
    }

    fn agent_node(id: &str, kind: Kind) -> RailNode {
        node(
            id,
            NodeKind::Agent,
            false,
            Some(Status {
                kind,
                label: "status".into(),
                since: 0,
            }),
        )
    }

    fn agent(id: &str) -> Actor {
        Actor {
            kind: ActorKind::Agent,
            id: id.into(),
            parent: None,
        }
    }

    struct Harness {
        messages: Messages,
        bus: Bus,
        bus_events: Receiver<Event>,
        touches: Arc<Touches>,
    }

    impl Harness {
        fn new(dir: &Path, rail: Vec<RailNode>) -> Self {
            let bus = Bus::new();
            let bus_events = bus.subscribe();
            Self {
                messages: Messages::open(dir, bus.clone(), FakeRail::new(rail)).unwrap(),
                bus,
                bus_events,
                touches: Arc::new(Touches::in_memory().unwrap()),
            }
        }

        fn ctx(&self, actor: Actor) -> Ctx {
            Ctx {
                actor,
                bus: self.bus.clone(),
                touches: Arc::clone(&self.touches),
            }
        }

        async fn call_as(
            &self,
            actor: Actor,
            method: &str,
            params: Value,
        ) -> Result<Value, RpcError> {
            self.messages.call(&self.ctx(actor), method, params).await
        }

        async fn call(&self, method: &str, params: Value) -> Result<Value, RpcError> {
            self.call_as(Actor::user(), method, params).await
        }

        /// Event names, oldest first.
        fn events(&mut self) -> Vec<String> {
            let mut seen = Vec::new();
            while let Ok(event) = self.bus_events.try_recv() {
                let event = serde_json::to_value(&event.data).unwrap();
                seen.push(event["name"].as_str().unwrap().to_owned());
            }
            seen
        }

        fn touches(&self, id: u32) -> Vec<(Verb, String)> {
            self.touches
                .history(&item(id))
                .unwrap()
                .into_iter()
                .map(|t| (t.verb, t.actor.id))
                .collect()
        }
    }

    #[tokio::test]
    async fn b1_send_assigns_sequential_ids_from_is_the_caller_and_touches_wrote() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);

        let first = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();
        assert_eq!(first["id"], 1);
        assert_eq!(first["from"]["id"], "a");
        assert_eq!(first["to"], "b");
        assert_eq!(first["kind"], "note");
        assert_eq!(first["body"], "hi");
        assert_eq!(first["replyTo"], Value::Null);
        assert_eq!(first["status"], "pending");
        assert_eq!(first["reason"], Value::Null);
        assert!(first["at"].as_i64().unwrap() > 0, "at: {:?}", first["at"]);

        let second = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "question", "body": "ok?"}),
            )
            .await
            .unwrap();
        assert_eq!(second["id"], 2);
        assert_eq!(second["kind"], "question");

        assert_eq!(h.events(), ["message.sent", "message.sent"]);
        assert_eq!(h.touches(1), [(Verb::Wrote, "a".to_owned())]);
    }

    #[tokio::test]
    async fn b1_kind_other_than_note_or_question_is_invalid_params() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);

        let err = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "shout", "body": "hi"}),
            )
            .await
            .unwrap_err();

        assert_eq!(err.code, code::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn b1_a_to_that_names_no_actor_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![]);

        let err = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "ghost", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap_err();

        assert_eq!(err.code, code::NOT_FOUND);
    }

    #[tokio::test]
    async fn b1_a_to_that_is_a_terminal_a_group_or_the_sender_is_invalid_params() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(
            dir.path(),
            vec![
                node("t", NodeKind::Terminal, false, None),
                node("g", NodeKind::Group, false, None),
            ],
        );

        for to in ["t", "g", "a"] {
            let err = h
                .call_as(
                    agent("a"),
                    "message.send",
                    json!({"to": to, "kind": "note", "body": "hi"}),
                )
                .await
                .unwrap_err();
            assert_eq!(err.code, code::INVALID_PARAMS, "to {to}");
        }
    }

    #[tokio::test]
    async fn b1_a_meta_agent_a_promoted_group_can_receive() {
        let dir = tempfile::tempdir().unwrap();
        let meta = node(
            "m",
            NodeKind::Group,
            true,
            Some(Status {
                kind: Kind::Idle,
                label: "status".into(),
                since: 0,
            }),
        );
        let h = Harness::new(dir.path(), vec![meta]);

        let sent = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "m", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();

        assert_eq!(sent["to"], "m");
    }

    #[tokio::test]
    async fn b1_empty_or_oversized_body_is_invalid_params() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);

        let empty = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": ""}),
            )
            .await
            .unwrap_err();
        let big = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "x".repeat(8193)}),
            )
            .await
            .unwrap_err();

        assert_eq!(
            (empty.code, big.code),
            (code::INVALID_PARAMS, code::INVALID_PARAMS)
        );
    }

    #[tokio::test]
    async fn b1_a_reply_to_that_names_no_message_is_invalid_params() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);

        let err = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi", "replyTo": 9}),
            )
            .await
            .unwrap_err();

        assert_eq!(err.code, code::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn b1_reply_to_round_trips_on_a_successful_send() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(
            dir.path(),
            vec![agent_node("a", Kind::Idle), agent_node("b", Kind::Idle)],
        );
        let first = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();
        assert_eq!(first["id"], 1);

        let second = h
            .call_as(
                agent("b"),
                "message.send",
                json!({"to": "a", "kind": "note", "body": "ok", "replyTo": 1}),
            )
            .await
            .unwrap();

        assert_eq!(second["replyTo"], 1);
    }

    #[tokio::test]
    async fn b1_dropped_messages_do_not_count_toward_the_bound_of_32() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "drop"}),
        )
        .await
        .unwrap();
        for _ in 0..32 {
            let dropped = h
                .call_as(
                    agent("a"),
                    "message.send",
                    json!({"to": "b", "kind": "note", "body": "hi"}),
                )
                .await
                .unwrap();
            assert_eq!(dropped["status"], "dropped");
        }

        let still_accepted = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();

        assert_eq!(still_accepted["status"], "dropped");
    }

    #[tokio::test]
    async fn b1_a_receiver_that_is_done_or_error_is_conflict() {
        for kind in [Kind::Done, Kind::Error] {
            let dir = tempfile::tempdir().unwrap();
            let h = Harness::new(dir.path(), vec![agent_node("b", kind)]);

            let err = h
                .call_as(
                    agent("a"),
                    "message.send",
                    json!({"to": "b", "kind": "note", "body": "hi"}),
                )
                .await
                .unwrap_err();

            assert_eq!(err.code, code::CONFLICT, "{kind:?}");
        }
    }

    #[tokio::test]
    async fn b1_a_receiver_at_the_bound_of_32_is_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        for _ in 0..32 {
            h.call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();
        }

        let err = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap_err();

        assert_eq!(err.code, code::CONFLICT);
    }

    #[tokio::test]
    async fn b1_a_body_of_exactly_8192_bytes_is_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);

        let sent = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "x".repeat(8192)}),
            )
            .await
            .unwrap();

        assert_eq!(sent["status"], "pending");
    }

    #[tokio::test]
    async fn b1_held_messages_count_toward_the_bound_of_32() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "ask-first"}),
        )
        .await
        .unwrap();
        for _ in 0..32 {
            h.call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();
        }

        let err = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap_err();

        assert_eq!(err.code, code::CONFLICT);
    }

    #[tokio::test]
    async fn b1_a_message_to_the_user_is_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![]);

        let sent = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "you", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();

        assert_eq!(sent["to"], "you");
    }

    #[tokio::test]
    async fn b10_a_message_to_the_user_is_delivered_at_once_and_never_fills_their_slots() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = Harness::new(dir.path(), vec![]);

        let sent = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "you", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();

        assert_eq!(sent["status"], "delivered");
        assert_eq!(sent["reason"], Value::Null);
        assert_eq!(h.events(), ["message.sent", "message.delivered"]);

        // 32 more Messages to the user, none of which ever becomes `pending` or `held`, so
        // a 33rd still succeeds: the bound never fills for the user (B10).
        for _ in 0..32 {
            h.call_as(
                agent("a"),
                "message.send",
                json!({"to": "you", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();
        }
        let still_accepted = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "you", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();
        assert_eq!(still_accepted["status"], "delivered");
    }

    #[tokio::test]
    async fn b1_a_rejected_call_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);

        let rejected = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "shout", "body": "hi"}),
            )
            .await
            .unwrap_err();
        assert_eq!(rejected.code, code::INVALID_PARAMS);
        assert_eq!(h.events(), Vec::<String>::new());

        let first = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();
        assert_eq!(first["id"], 1, "the rejected call used no id");
        assert_eq!(
            h.touches(1),
            [(Verb::Wrote, "a".to_owned())],
            "no Touch for the rejected call"
        );
    }

    #[tokio::test]
    async fn b3_an_ask_first_route_holds_the_message_with_reason_ask_first() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "ask-first"}),
        )
        .await
        .unwrap();
        h.events();

        let sent = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();

        assert_eq!(sent["status"], "held");
        assert_eq!(sent["reason"], "ask-first");
        assert_eq!(h.events(), ["message.sent", "message.held"]);
    }

    #[tokio::test]
    async fn b3_deliver_and_drop_are_the_users_calls_only() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "ask-first"}),
        )
        .await
        .unwrap();
        h.call_as(
            agent("a"),
            "message.send",
            json!({"to": "b", "kind": "note", "body": "hi"}),
        )
        .await
        .unwrap();

        let forbidden_deliver = h
            .call_as(agent("a"), "message.deliver", json!({"id": 1}))
            .await
            .unwrap_err();
        let forbidden_drop = h
            .call_as(agent("a"), "message.drop", json!({"id": 1}))
            .await
            .unwrap_err();
        assert_eq!(
            (forbidden_deliver.code, forbidden_drop.code),
            (code::FORBIDDEN, code::FORBIDDEN)
        );

        let delivered = h.call("message.deliver", json!({"id": 1})).await.unwrap();
        assert_eq!(delivered["status"], "pending");
    }

    #[tokio::test]
    async fn b3_deliver_or_drop_on_a_message_that_is_not_held_is_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        h.call_as(
            agent("a"),
            "message.send",
            json!({"to": "b", "kind": "note", "body": "hi"}),
        )
        .await
        .unwrap();

        let deliver_err = h
            .call("message.deliver", json!({"id": 1}))
            .await
            .unwrap_err();
        let drop_err = h.call("message.drop", json!({"id": 1})).await.unwrap_err();

        assert_eq!(
            (deliver_err.code, drop_err.code),
            (code::CONFLICT, code::CONFLICT)
        );
    }

    #[tokio::test]
    async fn b3_deliver_clears_the_held_reason() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "ask-first"}),
        )
        .await
        .unwrap();
        h.call_as(
            agent("a"),
            "message.send",
            json!({"to": "b", "kind": "note", "body": "hi"}),
        )
        .await
        .unwrap();

        let delivered = h.call("message.deliver", json!({"id": 1})).await.unwrap();

        assert_eq!(delivered["status"], "pending");
        assert_eq!(delivered["reason"], Value::Null);
    }

    #[tokio::test]
    async fn b3_drop_clears_the_held_reason() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "ask-first"}),
        )
        .await
        .unwrap();
        h.call_as(
            agent("a"),
            "message.send",
            json!({"to": "b", "kind": "note", "body": "hi"}),
        )
        .await
        .unwrap();

        let dropped = h.call("message.drop", json!({"id": 1})).await.unwrap();

        assert_eq!(dropped["status"], "dropped");
        assert_eq!(dropped["reason"], Value::Null);
    }

    #[tokio::test]
    async fn b3_deliver_or_drop_on_an_unknown_id_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![]);

        let deliver_err = h
            .call("message.deliver", json!({"id": 999}))
            .await
            .unwrap_err();
        let drop_err = h
            .call("message.drop", json!({"id": 999}))
            .await
            .unwrap_err();

        assert_eq!(
            (deliver_err.code, drop_err.code),
            (code::NOT_FOUND, code::NOT_FOUND)
        );
    }

    #[tokio::test]
    async fn b3_drop_moves_a_held_message_to_dropped_and_emits_message_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "ask-first"}),
        )
        .await
        .unwrap();
        h.call_as(
            agent("a"),
            "message.send",
            json!({"to": "b", "kind": "note", "body": "hi"}),
        )
        .await
        .unwrap();
        h.events();

        let dropped = h.call("message.drop", json!({"id": 1})).await.unwrap();

        assert_eq!(dropped["status"], "dropped");
        assert_eq!(h.events(), ["message.dropped"]);
    }

    #[tokio::test]
    async fn b4_a_drop_route_drops_the_message_without_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "drop"}),
        )
        .await
        .unwrap();
        h.events();

        let sent = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();

        assert_eq!(sent["status"], "dropped");
        assert_eq!(h.events(), ["message.sent", "message.dropped"]);
    }

    #[tokio::test]
    async fn b5_the_default_route_is_auto() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);

        let sent = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();

        assert_eq!(sent["status"], "pending");
        assert_eq!(h.call("route.list", Value::Null).await.unwrap(), json!([]));
    }

    #[tokio::test]
    async fn b5_route_set_is_the_users_call_and_route_list_returns_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = Harness::new(dir.path(), vec![]);

        let forbidden = h
            .call_as(
                agent("a"),
                "route.set",
                json!({"from": "a", "to": "b", "delivery": "drop"}),
            )
            .await
            .unwrap_err();
        assert_eq!(forbidden.code, code::FORBIDDEN);

        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "ask-first"}),
        )
        .await
        .unwrap();
        assert_eq!(h.events(), ["route.changed"]);

        let routes = h.call("route.list", Value::Null).await.unwrap();
        assert_eq!(
            routes,
            json!([{"from": "a", "to": "b", "delivery": "ask-first"}])
        );
    }

    #[tokio::test]
    async fn b5_a_route_to_the_user_or_from_itself_is_invalid_params() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![]);

        let to_user = h
            .call(
                "route.set",
                json!({"from": "a", "to": "you", "delivery": "drop"}),
            )
            .await
            .unwrap_err();
        let self_route = h
            .call(
                "route.set",
                json!({"from": "a", "to": "a", "delivery": "drop"}),
            )
            .await
            .unwrap_err();

        assert_eq!(
            (to_user.code, self_route.code),
            (code::INVALID_PARAMS, code::INVALID_PARAMS)
        );
    }

    #[tokio::test]
    async fn b5_a_route_change_does_not_affect_an_already_pending_or_held_message() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        let pending = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();
        assert_eq!(pending["status"], "pending");

        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "ask-first"}),
        )
        .await
        .unwrap();

        let still_pending = h.call("message.get", json!({"id": 1})).await.unwrap();
        assert_eq!(still_pending["status"], "pending");
    }

    #[tokio::test]
    async fn b5_a_route_change_does_not_affect_an_already_held_message() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "ask-first"}),
        )
        .await
        .unwrap();
        let held = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();
        assert_eq!(held["status"], "held");

        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "auto"}),
        )
        .await
        .unwrap();

        let still_held = h.call("message.get", json!({"id": 1})).await.unwrap();
        assert_eq!(still_held["status"], "held");
    }

    #[tokio::test]
    async fn b5_changing_an_existing_route_applies_to_messages_sent_after_it() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "ask-first"}),
        )
        .await
        .unwrap();

        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "drop"}),
        )
        .await
        .unwrap();
        assert_eq!(
            h.call("route.list", Value::Null).await.unwrap(),
            json!([{"from": "a", "to": "b", "delivery": "drop"}]),
            "the stored Route must have been updated, not left as ask-first"
        );

        let sent = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();
        assert_eq!(sent["status"], "dropped");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn b1_concurrent_sends_at_the_bound_never_both_succeed() {
        for _ in 0..40 {
            let dir = tempfile::tempdir().unwrap();
            let bus = Bus::new();
            let touches = Arc::new(Touches::in_memory().unwrap());
            let messages = Arc::new(
                Messages::open(
                    dir.path(),
                    bus.clone(),
                    FakeRail::new(vec![agent_node("b", Kind::Idle)]),
                )
                .unwrap(),
            );
            let make_ctx = |actor: Actor| Ctx {
                actor,
                bus: bus.clone(),
                touches: Arc::clone(&touches),
            };
            for _ in 0..31 {
                messages
                    .call(
                        &make_ctx(agent("a")),
                        "message.send",
                        json!({"to": "b", "kind": "note", "body": "hi"}),
                    )
                    .await
                    .unwrap();
            }

            let m1 = Arc::clone(&messages);
            let c1 = make_ctx(agent("a"));
            let m2 = Arc::clone(&messages);
            let c2 = make_ctx(agent("a"));
            let (r1, r2) = tokio::join!(
                tokio::spawn(async move {
                    m1.call(
                        &c1,
                        "message.send",
                        json!({"to": "b", "kind": "note", "body": "x"}),
                    )
                    .await
                }),
                tokio::spawn(async move {
                    m2.call(
                        &c2,
                        "message.send",
                        json!({"to": "b", "kind": "note", "body": "y"}),
                    )
                    .await
                }),
            );
            let successes = [r1.unwrap(), r2.unwrap()]
                .into_iter()
                .filter(Result::is_ok)
                .count();
            assert_eq!(
                successes, 1,
                "exactly one of two sends racing for the last open slot should pass"
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn b3_concurrent_deliver_and_drop_of_one_held_message_never_both_succeed() {
        for _ in 0..40 {
            let dir = tempfile::tempdir().unwrap();
            let bus = Bus::new();
            let touches = Arc::new(Touches::in_memory().unwrap());
            let messages = Arc::new(
                Messages::open(
                    dir.path(),
                    bus.clone(),
                    FakeRail::new(vec![agent_node("b", Kind::Idle)]),
                )
                .unwrap(),
            );
            let make_ctx = |actor: Actor| Ctx {
                actor,
                bus: bus.clone(),
                touches: Arc::clone(&touches),
            };
            messages
                .call(
                    &make_ctx(Actor::user()),
                    "route.set",
                    json!({"from": "a", "to": "b", "delivery": "ask-first"}),
                )
                .await
                .unwrap();
            messages
                .call(
                    &make_ctx(agent("a")),
                    "message.send",
                    json!({"to": "b", "kind": "note", "body": "hi"}),
                )
                .await
                .unwrap();

            let m1 = Arc::clone(&messages);
            let c1 = make_ctx(Actor::user());
            let m2 = Arc::clone(&messages);
            let c2 = make_ctx(Actor::user());
            let (r1, r2) = tokio::join!(
                tokio::spawn(
                    async move { m1.call(&c1, "message.deliver", json!({"id": 1})).await }
                ),
                tokio::spawn(async move { m2.call(&c2, "message.drop", json!({"id": 1})).await }),
            );
            let successes = [r1.unwrap(), r2.unwrap()]
                .into_iter()
                .filter(Result::is_ok)
                .count();
            assert_eq!(
                successes, 1,
                "a held Message must be delivered or dropped exactly once, never both"
            );
        }
    }

    #[tokio::test]
    async fn b8_routes_messages_and_statuses_survive_reopening_and_the_id_sequence_continues() {
        let dir = tempfile::tempdir().unwrap();
        let rail = vec![
            agent_node("b", Kind::Idle),
            agent_node("c", Kind::Idle),
            agent_node("d", Kind::Idle),
        ];
        {
            let h = Harness::new(dir.path(), rail.clone());
            h.call(
                "route.set",
                json!({"from": "a", "to": "b", "delivery": "ask-first"}),
            )
            .await
            .unwrap();
            h.call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();
            h.call_as(
                agent("a"),
                "message.send",
                json!({"to": "c", "kind": "note", "body": "still waiting"}),
            )
            .await
            .unwrap();
            h.call(
                "route.set",
                json!({"from": "a", "to": "d", "delivery": "drop"}),
            )
            .await
            .unwrap();
            h.call_as(
                agent("a"),
                "message.send",
                json!({"to": "d", "kind": "note", "body": "never typed"}),
            )
            .await
            .unwrap();
        }

        let h = Harness::new(dir.path(), rail);
        let message = h.call("message.get", json!({"id": 1})).await.unwrap();
        assert_eq!(message["status"], "held");
        assert_eq!(message["reason"], "ask-first");
        let pending = h.call("message.get", json!({"id": 2})).await.unwrap();
        assert_eq!(pending["status"], "pending");
        let dropped = h.call("message.get", json!({"id": 3})).await.unwrap();
        assert_eq!(dropped["status"], "dropped");
        let routes = h.call("route.list", Value::Null).await.unwrap();
        assert_eq!(
            routes,
            json!([
                {"from": "a", "to": "b", "delivery": "ask-first"},
                {"from": "a", "to": "d", "delivery": "drop"},
            ])
        );

        let next = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "again"}),
            )
            .await
            .unwrap();
        assert_eq!(next["id"], 4);
    }

    #[tokio::test]
    async fn b11_send_deliver_and_drop_touch_wrote_get_touches_read_list_touches_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "ask-first"}),
        )
        .await
        .unwrap();
        h.call_as(
            agent("a"),
            "message.send",
            json!({"to": "b", "kind": "note", "body": "hi"}),
        )
        .await
        .unwrap();
        h.call("message.deliver", json!({"id": 1})).await.unwrap();
        h.call_as(agent("b"), "message.get", json!({"id": 1}))
            .await
            .unwrap();
        h.call_as(agent("b"), "message.list", json!({}))
            .await
            .unwrap();

        assert_eq!(
            h.touches(1),
            [
                (Verb::Wrote, "a".to_owned()),
                (Verb::Wrote, "you".to_owned()),
                (Verb::Read, "b".to_owned()),
            ]
        );
    }

    #[tokio::test]
    async fn b11_drop_touches_wrote_by_the_user() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "ask-first"}),
        )
        .await
        .unwrap();
        h.call_as(
            agent("a"),
            "message.send",
            json!({"to": "b", "kind": "note", "body": "hi"}),
        )
        .await
        .unwrap();

        h.call("message.drop", json!({"id": 1})).await.unwrap();

        assert_eq!(
            h.touches(1),
            [
                (Verb::Wrote, "a".to_owned()),
                (Verb::Wrote, "you".to_owned()),
            ]
        );
    }

    #[tokio::test]
    async fn b11_provenance_touched_lists_a_message_under_each_actor_that_touched_it() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "ask-first"}),
        )
        .await
        .unwrap();
        h.call_as(
            agent("a"),
            "message.send",
            json!({"to": "b", "kind": "note", "body": "hi"}),
        )
        .await
        .unwrap();
        h.call("message.deliver", json!({"id": 1})).await.unwrap();

        let by_sender: Vec<_> = h
            .touches
            .touched("a")
            .unwrap()
            .into_iter()
            .map(|t| (t.verb, t.item))
            .collect();
        let by_user: Vec<_> = h
            .touches
            .touched("you")
            .unwrap()
            .into_iter()
            .map(|t| (t.verb, t.item))
            .collect();

        assert_eq!(by_sender, [(Verb::Wrote, "message:1".to_owned())]);
        assert_eq!(by_user, [(Verb::Wrote, "message:1".to_owned())]);
    }

    #[tokio::test]
    async fn b11_reading_is_by_actor_the_user_may_read_any_an_agent_only_its_own() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(
            dir.path(),
            vec![agent_node("b", Kind::Idle), agent_node("c", Kind::Idle)],
        );
        h.call_as(
            agent("a"),
            "message.send",
            json!({"to": "b", "kind": "note", "body": "hi"}),
        )
        .await
        .unwrap();

        let forbidden = h
            .call_as(agent("c"), "message.get", json!({"id": 1}))
            .await
            .unwrap_err();
        assert_eq!(forbidden.code, code::FORBIDDEN);

        h.call_as(agent("a"), "message.get", json!({"id": 1}))
            .await
            .unwrap();
        h.call_as(agent("b"), "message.get", json!({"id": 1}))
            .await
            .unwrap();
        h.call("message.get", json!({"id": 1})).await.unwrap();
    }

    #[tokio::test]
    async fn b11_list_for_an_agent_returns_only_messages_it_sent_or_received() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(
            dir.path(),
            vec![agent_node("b", Kind::Idle), agent_node("c", Kind::Idle)],
        );
        h.call_as(
            agent("a"),
            "message.send",
            json!({"to": "b", "kind": "note", "body": "hi"}),
        )
        .await
        .unwrap();
        h.call_as(
            agent("c"),
            "message.send",
            json!({"to": "b", "kind": "note", "body": "yo"}),
        )
        .await
        .unwrap();

        let listed_by_a = h
            .call_as(agent("a"), "message.list", json!({}))
            .await
            .unwrap();
        let ids: Vec<_> = listed_by_a
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["id"].clone())
            .collect();
        assert_eq!(ids, [json!(1)]);

        let listed_by_b = h
            .call_as(agent("b"), "message.list", json!({}))
            .await
            .unwrap();
        assert_eq!(listed_by_b.as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn b11_list_to_filters_by_receiver() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(
            dir.path(),
            vec![agent_node("b", Kind::Idle), agent_node("c", Kind::Idle)],
        );
        h.call_as(
            agent("a"),
            "message.send",
            json!({"to": "b", "kind": "note", "body": "hi"}),
        )
        .await
        .unwrap();
        h.call_as(
            agent("a"),
            "message.send",
            json!({"to": "c", "kind": "note", "body": "yo"}),
        )
        .await
        .unwrap();

        let listed = h.call("message.list", json!({"to": "b"})).await.unwrap();
        let ids: Vec<_> = listed
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["id"].clone())
            .collect();

        assert_eq!(ids, [json!(1)]);
    }

    #[tokio::test]
    async fn b11_list_status_filters_by_status() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "ask-first"}),
        )
        .await
        .unwrap();
        h.call_as(
            agent("a"),
            "message.send",
            json!({"to": "b", "kind": "note", "body": "held"}),
        )
        .await
        .unwrap();
        h.call_as(
            agent("a"),
            "message.send",
            json!({"to": "b", "kind": "note", "body": "also held"}),
        )
        .await
        .unwrap();
        h.call("message.deliver", json!({"id": 1})).await.unwrap();

        let listed = h
            .call("message.list", json!({"status": "pending"}))
            .await
            .unwrap();
        let ids: Vec<_> = listed
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["id"].clone())
            .collect();

        assert_eq!(ids, [json!(1)]);
    }
}
