//! B50, B51: a `note` is written to the receiver's Claude inbox, never typed. A failed check holds
//! it for `inbox` and shows on the Rail; a later passing check releases it.

use agents::claude_code::inbox::Entry;
use contracts::agent::Inbox;

use super::*;

/// What the adapter exposes for an Agent's Claude inbox; the Daemon passes `Agents`.
#[async_trait]
pub trait NoteInbox: Send + Sync {
    /// B51: check the inbox of `agent` and show the outcome on the Rail.
    async fn check(&self, agent: &str) -> Inbox;
    /// B50: write `notes` as entries; the answer names the failed state.
    fn write(&self, agent: &str, notes: &[Entry]) -> Result<(), Inbox>;
}

#[async_trait]
impl NoteInbox for agents::Agents {
    async fn check(&self, agent: &str) -> Inbox {
        self.inbox_check(agent).await
    }

    fn write(&self, agent: &str, notes: &[Entry]) -> Result<(), Inbox> {
        self.inbox_write(agent, notes)
    }
}

/// B50(e): a Message from the user is typed; one from an Agent, an Extension or `rupd` is a note
/// when its kind is `note`.
pub(crate) fn is_note(message: &Message) -> bool {
    message.kind == MessageKind::Note && message.from.kind != ActorKind::User
}

/// B51(d): the check at an `idle`; when it passes, what is held for `inbox` becomes `pending`.
pub(crate) async fn check_and_release(inner: &Inner, inbox: &dyn NoteInbox, agent: &str) -> Inbox {
    let state = inbox.check(agent).await;
    if state == Inbox::Ok {
        lock(&inner.store)
            .release_inbox_held(agent)
            .expect("message store");
    }
    state
}

/// B51(e): `message.deliver` of a held `inbox` note by hand needs the inbox to pass the check.
pub(crate) async fn require_usable(inner: &Inner, agent: &str) -> Result<(), RpcError> {
    let usable = match inner.inbox.get() {
        Some(inbox) => inbox.check(agent).await == Inbox::Ok,
        None => false,
    };
    if usable {
        Ok(())
    } else {
        Err(RpcError::new(
            code::INBOX_UNAVAILABLE,
            "the Agent's Claude inbox is missing or does not match this Claude Code",
        ))
    }
}

/// B51(b): holds each of `notes` for `inbox`, `message.held` once each.
fn hold(inner: &Inner, notes: &[Message]) {
    for note in notes {
        let held = lock(&inner.store)
            .hold_for_inbox(note.id)
            .expect("message store");
        if let Some(held) = held {
            inner
                .bus
                .emit(Actor::daemon(), EventData::MessageHeld(held.into()));
        }
    }
}

/// B50(a), B50(c): writes `notes` to `agent`'s inbox when `state` is `ok` and holds them for
/// `inbox` otherwise. A note already written and not yet recorded is not written again. `false`
/// when the batch must not go on (a stale revision or a Message no longer `pending`).
pub(crate) async fn send(
    inner: &Arc<Inner>,
    agent: &str,
    at: store::Binding,
    pick: Pick,
    mut notes: Vec<Message>,
    state: Inbox,
) -> bool {
    {
        let store = lock(&inner.store);
        notes.retain(|m| !store.in_flight(m.id).expect("message store"));
    }
    if notes.is_empty() {
        return true;
    }
    if state != Inbox::Ok {
        hold(inner, &notes);
        return true;
    }
    let (mut entries, mut texts) = (Vec::new(), pushes::bodies_of(&notes).into_iter());
    for note in &notes {
        let Ok(from) = sender_name(inner, &note.from).await else {
            return false;
        };
        let text = texts.next().unwrap_or_default();
        entries.push(Entry {
            id: note.id,
            from,
            one_line: text.starts_with("[wake]") || text.starts_with("[digest]"),
            text,
        });
    }
    let ids: Vec<u32> = notes.iter().map(|m| m.id).collect();
    let _typing = {
        let mut store = lock(&inner.store);
        if store.require_generation(agent, at).is_err() {
            return false;
        }
        for note in &notes {
            let bound = store.bound(note.id).expect("message store");
            if bound.0 != at.0 || (pick == Pick::Next && bound.1 >= at.1) {
                return false;
            }
        }
        {
            let mut typing = lock(&inner.typing);
            if ids.iter().any(|id| typing.contains(id)) {
                return false;
            }
            typing.extend(ids.iter().copied());
        }
        let guard = Typing {
            set: &inner.typing,
            ids: ids.clone(),
        };
        if store
            .record_flight_all(&ids)
            .expect("message store")
            .is_none()
        {
            return false;
        }
        guard
    };
    // A note with nothing left to say (B34) has no entry to wait for.
    let (empty, said): (Vec<Entry>, Vec<Entry>) =
        entries.into_iter().partition(|entry| entry.text.is_empty());
    let written = match inner.inbox.get() {
        Some(inbox) if !said.is_empty() => inbox.write(agent, &said),
        _ => Ok(()),
    };
    if written.is_err() {
        for id in &ids {
            lock(&inner.store).clear_flight(*id).expect("message store");
        }
        hold(inner, &notes);
        return true;
    }
    if !empty.is_empty() {
        let empty: Vec<u32> = empty.iter().map(|entry| entry.id).collect();
        confirm_ids(inner, agent, &empty);
    }
    true
}
