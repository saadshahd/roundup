//! Messages between Actors, and the Routes that decide how they are delivered. Owner: messages
//! Builder. Stores Messages and Routes, answers the user's and an Actor's calls, and on an `idle`
//! hands one Message to `Deliver` (slice 3 maps that to H11's `Agents::prompt`).

mod store;

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use contracts::agent::{NodeKind, RailNode};
use contracts::message::{
    Delivery, ListParams, Message, MessageId, MessageKind, MessageStatus, Reason, Route,
    SendParams, SetRouteParams, TakeoverChanged, TakeoverParams,
};
use contracts::{Actor, ActorKind, EventData, Kind, Verb};
use rpc::{Bus, Ctx, Module, OpenError, RpcError, code, params, reply};
use serde_json::Value;
use store::Store;
use tokio::sync::broadcast::error::RecvError;

const MAX_BODY_BYTES: usize = 8192;
const OPEN_BOUND: u32 = 32;

/// Types a Message's text into an Agent's Terminal: the Agent id and the text. Slice 3 maps
/// `Agents::prompt` onto this; tests pass a fake that records calls.
pub type Deliver = Arc<
    dyn Fn(String, String) -> Pin<Box<dyn Future<Output = Result<(), Refusal>> + Send>>
        + Send
        + Sync,
>;

/// Why `deliver` could not type a Message, from `Agents::prompt` (B2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Busy,
    NotAccepted,
    NotFound,
}

struct Inner {
    store: Mutex<Store>,
    /// Answers `rail.tree`, the only thing `message.send` needs to know about a receiver: whether
    /// it names an Agent (or a Door) at all, and that Agent's Kind.
    agents: Arc<dyn Module>,
    bus: Bus,
    deliver: Deliver,
    /// Starts the listener at most once: immediately when `open` already runs on a Tokio
    /// runtime, lazily on the first call otherwise (a caller that builds a `Messages` before
    /// its own runtime exists).
    listener_started: std::sync::Once,
    /// Subscribed at `open`, before any runtime may exist, so no `agent.status` fires between
    /// `open` and the listener's first poll; `start_listener` takes it out at most once.
    events: Mutex<Option<tokio::sync::broadcast::Receiver<contracts::Event>>>,
    /// A `Ctx` for the calls the listener makes on its own behalf (B9's resync), never the
    /// caller's: nothing reads this log.
    touches: Arc<provenance::Touches>,
}

pub struct Messages {
    inner: Arc<Inner>,
}

impl Messages {
    /// `dir` is the Project's `.roundup/` directory. `bus` is for events no call caused, and is
    /// how this module learns a Kind changed (it reads `agent.status`); `deliver` types a
    /// Message's text into an Agent's Terminal.
    pub fn open(
        dir: &Path,
        bus: Bus,
        agents: Arc<dyn Module>,
        deliver: Deliver,
    ) -> Result<Self, OpenError> {
        let events = bus.subscribe();
        let inner = Arc::new(Inner {
            store: Mutex::new(Store::open(&dir.join("messages.db"))?),
            agents,
            bus,
            deliver,
            listener_started: std::sync::Once::new(),
            events: Mutex::new(Some(events)),
            touches: Arc::new(provenance::Touches::in_memory()?),
        });
        if tokio::runtime::Handle::try_current().is_ok() {
            start_listener(&inner);
        }
        Ok(Self { inner })
    }

    async fn send(&self, ctx: &Ctx, p: SendParams) -> Result<Value, RpcError> {
        if is_receiver(&ctx.actor, &p.to) {
            return Err(RpcError::new(
                code::INVALID_PARAMS,
                "a Message cannot be sent to its own sender",
            ));
        }
        let receiver_status = self.resolve_receiver(&p.to).await?;
        if p.body.is_empty() || p.body.len() > MAX_BODY_BYTES {
            return Err(RpcError::new(
                code::INVALID_PARAMS,
                format!("body must be 1 to {MAX_BODY_BYTES} bytes"),
            ));
        }
        if let Some(status) = &receiver_status
            && ended(status)
        {
            return Err(RpcError::conflict(format!(
                "{} cannot receive a Message: its Kind is {:?}",
                p.to, status.status
            )));
        }
        // The reply-to check, the bound check and the insert run while one lock is held, so a
        // second sender racing for the same receiver's last open slot cannot pass its own check
        // before this one's insert lands (B1).
        let message = {
            let mut store = self.store()?;
            if let Some(node) = &receiver_status {
                accept_receiver(&self.inner, &mut store, node)?;
            }
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
                let route = store
                    .get_route(&ctx.actor.id, &p.to)?
                    .unwrap_or(Delivery::Auto);
                // B6: under a Takeover, an auto Message from any Actor but the user is held for
                // it; the user's own Messages are delivered as B2 says, Takeover or not.
                let held_for_takeover = ctx.actor.kind != ActorKind::User
                    && route == Delivery::Auto
                    && store.is_takeover_active(&p.to);
                match route {
                    _ if held_for_takeover => (MessageStatus::Held, Some(Reason::Takeover)),
                    Delivery::Auto => (MessageStatus::Pending, None),
                    Delivery::AskFirst => (MessageStatus::Held, Some(Reason::AskFirst)),
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
                receiver_status.as_ref().map_or((0, 0), binding),
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
            messages.retain(|message| {
                is_sender(message, &ctx.actor) || is_receiver(&ctx.actor, &message.to)
            });
        }
        reply(&messages)
    }

    /// B3, B6: moves a `held` Message to `pending`; the next `idle` types it (B2). B9: a Message
    /// held for an Agent that has since ended is `CONFLICT` and stays `held`.
    async fn deliver(&self, ctx: &Ctx, p: MessageId) -> Result<Value, RpcError> {
        require_user(ctx, "deliver a held Message")?;
        let message = self.get_or_not_found(p.id)?;
        let receiver = self.resolve_receiver(&message.to).await?;
        {
            let mut store = self.store()?;
            if let Some(node) = &receiver {
                accept_receiver(&self.inner, &mut store, node)?;
            }
            if !store.release_held(p.id, MessageStatus::Pending, None)? {
                return Err(RpcError::conflict("Message is not held"));
            }
            if let Some(node) = &receiver {
                store.bind(p.id, binding(node))?;
            }
        }
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

    /// B6: starts a Takeover of `agent`. A repeated call, while one is already active, changes
    /// nothing and is not an error.
    async fn takeover_begin(&self, ctx: &Ctx, p: TakeoverParams) -> Result<Value, RpcError> {
        require_user(ctx, "begin a Takeover")?;
        let TakeoverParams { agent } = p;
        let node = self
            .resolve_takeover_target(&agent)
            .await?
            .expect("Agent receiver");
        let held = {
            let mut store = self.store()?;
            accept_receiver(&self.inner, &mut store, &node)?;
            store.begin_takeover(&agent)?
        };
        if let Some(held) = held {
            for message in &held {
                ctx.emit(EventData::MessageHeld(message.clone()));
            }
            ctx.emit(EventData::TakeoverChanged(TakeoverChanged {
                agent,
                on: true,
            }));
        }
        Ok(Value::Null)
    }

    /// B6: ends a Takeover of `agent`. A repeated call, or one with no Takeover active, changes
    /// nothing and is not an error.
    async fn takeover_end(&self, ctx: &Ctx, p: TakeoverParams) -> Result<Value, RpcError> {
        require_user(ctx, "end a Takeover")?;
        let TakeoverParams { agent } = p;
        let node = self
            .resolve_takeover_target(&agent)
            .await?
            .expect("Agent receiver");
        let promoted = {
            let mut store = self.store()?;
            accept_receiver(&self.inner, &mut store, &node)?;
            store.end_takeover(&agent)?
        };
        if promoted.is_some() {
            ctx.emit(EventData::TakeoverChanged(TakeoverChanged {
                agent,
                on: false,
            }));
        }
        Ok(Value::Null)
    }

    /// `agent`'s Status for a Takeover call (B6): `NOT_FOUND` unless `agent` names an Agent node
    /// on the Rail or a Door — never the user, a Terminal or a plain Room.
    async fn resolve_takeover_target(&self, agent: &str) -> Result<Option<RailNode>, RpcError> {
        let nodes = rail_nodes(&self.inner).await?;
        let node = nodes
            .into_iter()
            .find(|node| node.id == agent && (node.kind != NodeKind::Terminal))
            .ok_or_else(|| RpcError::not_found(format!("actor {agent}")))?;
        Ok(Some(node))
    }

    /// `to`'s Status, when it names an Agent or a Door; `None` when it names the user.
    /// `NOT_FOUND` when it names no Actor; `INVALID_PARAMS` when it names a Terminal or a plain
    /// Room, neither of which can receive a Message.
    async fn resolve_receiver(&self, to: &str) -> Result<Option<RailNode>, RpcError> {
        if to == Actor::user().id {
            return Ok(None);
        }
        let nodes = rail_nodes(&self.inner).await?;
        let node = nodes
            .into_iter()
            .find(|node| node.id == to)
            .ok_or_else(|| RpcError::not_found(format!("actor {to}")))?;
        if node.kind == NodeKind::Terminal {
            return Err(RpcError::new(
                code::INVALID_PARAMS,
                format!("{to} cannot receive a Message: it is a {:?}", node.kind),
            ));
        }
        Ok(Some(node))
    }

    /// Moves a `held` Message to `status`, atomically with the check that it is still `held`
    /// (B3): two concurrent `deliver`/`drop` calls on the same Message can never both apply.
    /// `NOT_FOUND` when no such Message exists, `CONFLICT` when it exists but is not `held`.
    fn release_held(
        &self,
        id: u32,
        status: MessageStatus,
        reason: Option<Reason>,
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
        self.inner
            .store
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
        || is_sender(message, &ctx.actor)
        || is_receiver(&ctx.actor, &message.to)
    {
        return Ok(());
    }
    Err(RpcError::forbidden(format!(
        "{} may not read message {}",
        ctx.actor.id, message.id
    )))
}

/// Whether `actor` sent `message`: by kind and id together, never id alone, since an Agent and
/// an Extension draw their ids from different namespaces and could share one.
fn is_sender(message: &Message, actor: &Actor) -> bool {
    message.from.kind == actor.kind && message.from.id == actor.id
}

/// Whether `to`, a Message's receiver id, names `actor`: Agents and the user are the only
/// receivers, so an Extension that shares an Agent's id is not that Agent.
fn is_receiver(actor: &Actor, to: &str) -> bool {
    matches!(actor.kind, ActorKind::Agent | ActorKind::User) && actor.id == to
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
        &["message", "route", "takeover"]
    }

    async fn call(&self, ctx: &Ctx, method: &str, value: Value) -> Result<Value, RpcError> {
        start_listener(&self.inner);
        match method {
            "message.send" => self.send(ctx, params(value)?).await,
            "message.get" => self.get(ctx, params(value)?),
            "message.list" => self.list(ctx, params(value)?),
            "message.deliver" => self.deliver(ctx, params(value)?).await,
            "message.drop" => self.drop_message(ctx, params(value)?),
            "route.set" => self.set_route(ctx, params(value)?),
            "route.list" => self.list_routes(),
            "takeover.begin" => self.takeover_begin(ctx, params(value)?).await,
            "takeover.end" => self.takeover_end(ctx, params(value)?).await,
            _ => Err(RpcError::method_not_found(method)),
        }
    }
}

/// Starts the status listener at most once for this `Inner` (its `Once`), and only when a Tokio
/// runtime is there to run it on. The subscription itself was already taken at `open`, so no
/// `agent.status` fired between `open` and this first poll is lost.
fn start_listener(inner: &Arc<Inner>) {
    inner.listener_started.call_once(|| {
        let events = inner
            .events
            .lock()
            .unwrap()
            .take()
            .expect("subscribed once, at open");
        spawn_status_listener(Arc::clone(inner), events);
    });
}

/// Reads `agent.status` off `events` for as long as `inner` has a subscriber: an `idle` Kind
/// calls `deliver` once, for the oldest deliverable Message to that Agent (B2, B7); a `done` or
/// `error` Kind ends any Takeover of it and drops its `pending` Messages with the reason
/// `receiver gone` (B6, B9). A `Lagged` receiver may have missed one of those, so it resyncs from
/// the Rail instead of guessing what it lost (B9), and tries again on its next event while the
/// Rail cannot answer.
fn spawn_status_listener(
    inner: Arc<Inner>,
    mut events: tokio::sync::broadcast::Receiver<contracts::Event>,
) {
    tokio::spawn(async move {
        let mut resync_due = !resync(&inner).await;
        loop {
            match events.recv().await {
                Ok(contracts::Event {
                    data: EventData::AgentStatus(status),
                    ..
                }) => {
                    on_status(
                        &inner,
                        &status.id,
                        &status.incarnation,
                        &status.status_revision,
                        status.status.kind,
                    )
                    .await
                }
                Ok(contracts::Event {
                    data: EventData::RailChanged,
                    ..
                }) => resync_due = true,
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => resync_due = true,
                Err(RecvError::Closed) => break,
            }
            if resync_due {
                resync_due = !resync(&inner).await;
            }
        }
    });
}

async fn on_status(inner: &Arc<Inner>, agent: &str, incarnation: &str, revision: &str, kind: Kind) {
    let at = (
        contracts::agent::parse_positive_ordinal(incarnation).expect("wire Incarnation"),
        contracts::agent::parse_positive_ordinal(revision).expect("wire Status revision"),
    );
    {
        let mut store = inner.store.lock().unwrap();
        reconcile(
            inner,
            &mut store,
            agent,
            at,
            matches!(kind, Kind::Done | Kind::Error),
            false,
        )
        .expect("message store");
    }
    if kind == Kind::Idle {
        on_idle(inner, agent, at).await;
    }
}

/// Resync after bus lag or Rail changes so missed endings cannot leave Messages deliverable.
/// A failed Rail read is retried on the next event; it never guesses that an Agent recovered.
async fn resync(inner: &Arc<Inner>) -> bool {
    let known = {
        let store = inner.store.lock().unwrap();
        store
            .receivers()
            .expect("message store")
            .into_iter()
            .map(|id| {
                let generation = store.generation(&id).expect("message store");
                (id, generation)
            })
            .collect::<Vec<_>>()
    };
    let Ok(nodes) = rail_nodes(inner).await else {
        return false;
    };
    let mut store = inner.store.lock().unwrap();
    for (id, generation) in known {
        if !nodes.iter().any(|node| node.id == id)
            && store.generation(&id).expect("message store") == generation
        {
            let incarnation = generation.map_or(0, |state| state.incarnation);
            reconcile(inner, &mut store, &id, (incarnation, i64::MAX), true, true)
                .expect("message store");
        }
    }
    for node in nodes {
        reconcile(
            inner,
            &mut store,
            &node.id,
            binding(&node),
            ended(&node),
            permanently_ended(&node),
        )
        .expect("message store");
    }
    true
}

/// The Rail's nodes, read with the Daemon as the caller.
async fn rail_nodes(inner: &Inner) -> Result<Vec<RailNode>, RpcError> {
    let ctx = Ctx {
        actor: Actor::daemon(),
        bus: inner.bus.clone(),
        touches: Arc::clone(&inner.touches),
    };
    let tree = inner.agents.call(&ctx, "rail.tree", Value::Null).await?;

    serde_json::from_value(tree).map_err(RpcError::internal)
}

/// B2's `<sender's name>`: the Rail name for an Agent, since a Rail node can be renamed; the
/// Actor's id for the user (`you`), the Daemon and an Extension.
async fn sender_name(inner: &Inner, from: &Actor) -> Result<String, RpcError> {
    if from.kind != ActorKind::Agent {
        return Ok(from.id.clone());
    }
    let nodes = rail_nodes(inner).await?;

    Ok(nodes
        .into_iter()
        .find(|node| node.id == from.id)
        .map_or_else(|| from.id.clone(), |node| node.name))
}

/// B2, B7: types the oldest pending Message to `agent`, if any; the next `idle` picks up the
/// next one, so exactly one Message is typed per `idle`. It is recorded `delivered` in one
/// conditional write before typing starts (B8), so a Takeover beginning while the prompt is being
/// typed cannot hold a Message already in flight; a refusal reverts that record.
async fn on_idle(inner: &Arc<Inner>, agent: &str, at: store::Binding) {
    let found = {
        let store = inner.store.lock().unwrap();
        if store.require_generation(agent, at).is_err() {
            return;
        }
        store.next_pending(agent, at).expect("message store")
    };
    let Some(found) = found else {
        return;
    };
    // The name is read before the record: a Rail that cannot answer leaves the Message `pending`
    // for the next `idle`, where a read after the record would strand it `delivered` and untyped.
    let Ok(from) = sender_name(inner, &found.from).await else {
        return;
    };
    let recorded = {
        let store = inner.store.lock().unwrap();
        if store.require_generation(agent, at).is_err()
            || (store.bound(found.id).expect("message store").0 != at.0
                || store.bound(found.id).expect("message store").1 >= at.1)
        {
            return;
        }
        store.mark_delivered(found.id).expect("message store")
    };
    let Some(message) = recorded else {
        return;
    };
    let text = format!(
        "[from {from}, {}] {}",
        kind_label(message.kind),
        message.body
    );
    if (inner.deliver)(agent.to_owned(), text).await.is_err() {
        // Every refusal is slice 3's to tell apart; here each one undoes the record, back to `pending`, or `held` for `takeover` if one began meanwhile (B6).
        let reverted = inner
            .store
            .lock()
            .unwrap()
            .unmark_delivered(message.id)
            .expect("message store");
        if let Some(held) = reverted.filter(|m| m.status == MessageStatus::Held) {
            inner
                .bus
                .emit(Actor::daemon(), EventData::MessageHeld(held));
        }
        return;
    }
    inner
        .bus
        .emit(Actor::daemon(), EventData::MessageDelivered(message));
}

fn binding(node: &RailNode) -> store::Binding {
    (
        node.incarnation.as_deref().map_or(0, |n| {
            contracts::agent::parse_positive_ordinal(n).expect("Rail Incarnation")
        }),
        node.status_revision.as_deref().map_or(i64::MAX, |n| {
            contracts::agent::parse_positive_ordinal(n).expect("Rail Status revision")
        }),
    )
}

fn ended(node: &RailNode) -> bool {
    node.status
        .as_ref()
        .is_none_or(|s| matches!(s.kind, Kind::Done | Kind::Error))
}

fn permanently_ended(node: &RailNode) -> bool {
    node.status_revision.is_none() || (ended(node) && node.terminal_id.is_none())
}

fn accept_receiver(inner: &Inner, store: &mut Store, node: &RailNode) -> Result<(), RpcError> {
    reconcile(
        inner,
        store,
        &node.id,
        binding(node),
        ended(node),
        permanently_ended(node),
    )?;
    store.require_generation(&node.id, binding(node))
}

fn reconcile(
    inner: &Inner,
    store: &mut Store,
    agent: &str,
    at: store::Binding,
    closed: bool,
    permanent: bool,
) -> Result<(), RpcError> {
    let previous = store.generation(agent)?;
    let through = if permanent {
        Some((at.0, i64::MAX))
    } else if closed {
        Some(at)
    } else if previous.is_none_or(|state| state.incarnation < at.0) {
        Some((at.0.saturating_sub(1), i64::MAX))
    } else {
        None
    };
    if let Some(through) = through {
        if store
            .takeover_bound(agent)
            .is_some_and(|bound| bound <= through)
            && store.end_takeover(agent)?.is_some()
        {
            inner.bus.emit(
                Actor::daemon(),
                EventData::TakeoverChanged(TakeoverChanged {
                    agent: agent.to_owned(),
                    on: false,
                }),
            );
        }
        for message in store.drop_all_pending(agent, Reason::ReceiverGone, through)? {
            inner
                .bus
                .emit(Actor::daemon(), EventData::MessageDropped(message));
        }
    }
    if previous.is_some_and(|state| state.incarnation > at.0) {
        return Ok(());
    }
    let mut state = previous
        .filter(|state| state.incarnation == at.0)
        .unwrap_or(store::ReceiverState {
            incarnation: at.0,
            revision: at.1,
            closed_revision: -1,
            closed: false,
        });
    state.revision = state.revision.max(at.1);
    if closed {
        state.closed_revision = state.closed_revision.max(at.1);
    }
    state.closed |= permanent;
    store.set_generation(agent, state)
}

/// B2's `<kind>`: `note` or `question` as on the wire, never Rust's `Debug` spelling.
fn kind_label(kind: MessageKind) -> &'static str {
    match kind {
        MessageKind::Note => "note",
        MessageKind::Question => "question",
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

    mod b_held_tests;

    /// Stands in for the `agents` module's `rail.tree`: the only Rail fact `message.send` needs,
    /// without a real Agent, Terminal or Launcher.
    struct FakeRail(StdMutex<Vec<RailNode>>, std::sync::atomic::AtomicBool);

    impl FakeRail {
        fn new(nodes: Vec<RailNode>) -> Arc<Self> {
            Arc::new(Self(StdMutex::new(nodes), false.into()))
        }

        /// While `true`, `rail.tree` answers with an error.
        fn fail(&self, failing: bool) {
            self.1.store(failing, std::sync::atomic::Ordering::SeqCst);
        }
    }

    #[async_trait]
    impl Module for FakeRail {
        fn namespaces(&self) -> &'static [&'static str] {
            &["agent", "rail"]
        }

        async fn call(&self, _ctx: &Ctx, method: &str, _params: Value) -> Result<Value, RpcError> {
            match method {
                "rail.tree" if self.1.load(std::sync::atomic::Ordering::SeqCst) => {
                    Err(RpcError::internal("rail.tree is down"))
                }
                "rail.tree" => Ok(serde_json::to_value(&*self.0.lock().unwrap()).unwrap()),
                _ => Err(RpcError::method_not_found(method)),
            }
        }
    }

    fn node(id: &str, kind: NodeKind, _door: bool, status: Option<Status>) -> RailNode {
        RailNode {
            id: id.into(),
            kind,
            name: id.into(),
            parent: None,
            order: 0,
            status,
            incarnation: Some("1".into()),
            status_revision: Some("1".into()),
            terminal_id: None,
            worktree: None,
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

    /// A `Deliver` for tests that never trigger an `idle` event: it is never called.
    fn noop_deliver() -> Deliver {
        Arc::new(|_, _| Box::pin(async { Ok(()) }))
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
                messages: Messages::open(dir, bus.clone(), FakeRail::new(rail), noop_deliver())
                    .unwrap(),
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
                node("g", NodeKind::Room, false, None),
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
            assert_eq!(
                err.code,
                if to == "g" {
                    code::CONFLICT
                } else {
                    code::INVALID_PARAMS
                },
                "to {to}"
            );
        }
    }

    #[tokio::test]
    async fn b1_a_door_can_receive() {
        let dir = tempfile::tempdir().unwrap();
        let meta = node(
            "m",
            NodeKind::Room,
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
        assert!(err.message.contains("32"), "{}", err.message);
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
    async fn b1_the_bound_of_32_is_per_receiver_not_shared() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(
            dir.path(),
            vec![agent_node("b", Kind::Idle), agent_node("c", Kind::Idle)],
        );
        for _ in 0..32 {
            h.call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();
        }

        let to_c = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "c", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();

        assert_eq!(to_c["status"], "pending");
    }

    #[tokio::test]
    async fn b1_a_rejected_call_at_the_bound_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
        for _ in 0..32 {
            h.call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();
        }
        h.events();

        let err = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap_err();

        assert_eq!(err.code, code::CONFLICT);
        assert_eq!(
            h.events(),
            Vec::<String>::new(),
            "the bound check must run before the insert, so a rejected call emits no event"
        );
        let phantom = h.call("message.get", json!({"id": 33})).await.unwrap_err();
        assert_eq!(
            phantom.code,
            code::NOT_FOUND,
            "no row was ever inserted for the rejected call"
        );
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
    async fn b3_deliver_of_a_held_message_emits_no_event_until_it_is_typed() {
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

        h.call("message.deliver", json!({"id": 1})).await.unwrap();

        assert!(h.events().is_empty());
    }

    #[tokio::test]
    async fn b11_a_forbidden_get_leaves_no_touch() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("b", Kind::Idle)]);
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
        assert_eq!(h.touches(1), [(Verb::Wrote, "a".to_owned())]);
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

    #[tokio::test]
    async fn b5_a_route_applies_only_to_its_own_sender_not_every_sender_to_the_receiver() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(
            dir.path(),
            vec![agent_node("b", Kind::Idle), agent_node("c", Kind::Idle)],
        );
        h.call(
            "route.set",
            json!({"from": "a", "to": "b", "delivery": "drop"}),
        )
        .await
        .unwrap();

        let from_a = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();
        let from_c = h
            .call_as(
                agent("c"),
                "message.send",
                json!({"to": "b", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();

        assert_eq!(from_a["status"], "dropped", "a's Route to b is drop");
        assert_eq!(
            from_c["status"], "pending",
            "c has no Route to b, so the default auto applies, not a's drop Route"
        );
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
                    noop_deliver(),
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
                    noop_deliver(),
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
    async fn b11_list_for_an_agent_never_matches_an_extension_sharing_its_id() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("y", Kind::Idle)]);
        let ext_x = Actor {
            kind: ActorKind::Ext,
            id: "x".into(),
            parent: None,
        };
        h.call_as(
            ext_x,
            "message.send",
            json!({"to": "y", "kind": "note", "body": "hi"}),
        )
        .await
        .unwrap();

        let listed_by_agent_x = h
            .call_as(agent("x"), "message.list", json!({}))
            .await
            .unwrap();

        assert_eq!(
            listed_by_agent_x.as_array().unwrap().len(),
            0,
            "Agent x must not see the Extension x's Message just because the id strings match"
        );
    }

    #[tokio::test]
    async fn b11_reading_an_agent_never_matches_an_extension_sharing_its_id() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("y", Kind::Idle)]);
        let ext_x = Actor {
            kind: ActorKind::Ext,
            id: "x".into(),
            parent: None,
        };
        let sent = h
            .call_as(
                ext_x,
                "message.send",
                json!({"to": "y", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();
        let id = sent["id"].as_u64().unwrap() as u32;

        let err = h
            .call_as(agent("x"), "message.get", json!({"id": id}))
            .await
            .unwrap_err();

        assert_eq!(
            err.code,
            code::FORBIDDEN,
            "Agent x may not read a Message the Extension x sent, just because the id strings match"
        );
    }

    fn ext(id: &str) -> Actor {
        Actor {
            kind: ActorKind::Ext,
            id: id.into(),
            parent: None,
        }
    }

    #[tokio::test]
    async fn b11_an_extension_sharing_a_receivers_id_cannot_read_or_list_its_messages() {
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

        let got = h
            .call_as(ext("b"), "message.get", json!({"id": sent["id"]}))
            .await
            .unwrap_err();
        let listed = h
            .call_as(ext("b"), "message.list", json!({}))
            .await
            .unwrap();

        assert_eq!(got.code, code::FORBIDDEN);
        assert_eq!(listed.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn b1_only_an_agent_or_the_user_is_its_own_receiver() {
        let dir = tempfile::tempdir().unwrap();
        let h = Harness::new(dir.path(), vec![agent_node("a", Kind::Idle)]);
        let to_you = json!({"to": "you", "kind": "note", "body": "hi"});

        let user = h.call("message.send", to_you).await.unwrap_err();
        let agent_self = h
            .call_as(
                agent("a"),
                "message.send",
                json!({"to": "a", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap_err();
        let extension = h
            .call_as(
                ext("a"),
                "message.send",
                json!({"to": "a", "kind": "note", "body": "hi"}),
            )
            .await
            .unwrap();

        assert_eq!(
            (user.code, agent_self.code),
            (code::INVALID_PARAMS, code::INVALID_PARAMS)
        );
        assert_eq!(extension["status"], "pending");
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
