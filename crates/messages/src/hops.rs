//! B13 to B17: a `question` sent to a Door that does not answer passes up the Rail, one Door at a
//! time, and lands in the user's Inbox. This file decides what the next hop is; `lib.rs` stores
//! it, emits its events and logs its Touch.

use contracts::agent::{NodeKind, RailNode};
use contracts::message::{Delivery, Message, MessageKind, MessageStatus, Reason};
use contracts::{Actor, ActorKind};
use rpc::RpcError;

use super::store::{Chain, Store};
use super::{Inner, OPEN_BOUND, accept_receiver, binding, ended, send_status};

/// B14: how long a live Door has to answer a hop.
pub(crate) const HOP_BOUND_MS: i64 = 60_000;

/// B13: the Doors a question follows after `door`: the Rooms above it, nearest first. The sender
/// is never a hop, so `origin` is left out.
pub(crate) fn doors_above(nodes: &[RailNode], door: &str, origin: &str) -> Vec<String> {
    let parent_of = |id: &str| {
        nodes
            .iter()
            .find(|node| node.id == id)
            .and_then(|node| node.parent.clone())
    };
    let mut above = Vec::new();
    let mut at = parent_of(door);
    while let Some(id) = at.filter(|_| above.len() <= nodes.len()) {
        if id != origin
            && nodes
                .iter()
                .any(|node| node.id == id && node.kind == NodeKind::Room)
        {
            above.push(id.clone());
        }
        at = parent_of(&id);
    }
    above
}

/// What `settle` changed, for the caller to emit once the lock is released.
pub(crate) struct Made {
    pub message: Message,
    /// A hop or Landing the Daemon made: it has `message.sent` and a `wrote` Touch by `rupd`.
    pub new: bool,
}

/// B14, B16: moves every live chain whose current hop ended without an answer, ran out its bound
/// at `now`, or is `force`d (B13's `message.pass`) to its next hop, and on to the Landing when
/// no Door is left.
pub(crate) fn settle(
    inner: &Inner,
    store: &mut Store,
    nodes: &[RailNode],
    now: i64,
    force: Option<u32>,
) -> Result<Vec<Made>, RpcError> {
    store.arm_chains(now)?;
    let mut made = Vec::new();
    for chain in store.live_chains()? {
        let Some(hop) = store.get(chain.current)? else {
            continue;
        };
        let open = matches!(
            hop.status,
            MessageStatus::Pending | MessageStatus::Delivered
        );
        let gone = hop.status == MessageStatus::Delivered
            && nodes
                .iter()
                .find(|node| node.id == hop.to)
                .is_none_or(ended);
        let run_out = open && chain.armed_at.is_some_and(|at| at + HOP_BOUND_MS <= now);
        let passes = open && force == Some(hop.id);
        if hop.status != MessageStatus::Dropped && !gone && !run_out && !passes {
            continue;
        }
        // One transaction per chain: a kill cannot leave the next hop or Landing stored while the
        // chain still points at the hop it left (B8, B17).
        let moved = store.atomically(|store| {
            let mut moved = Vec::new();
            if let Some(passed) = store.drop_passed(hop.id)? {
                moved.push(Made {
                    message: passed,
                    new: false,
                });
            }
            advance(inner, store, nodes, now, &chain, &mut moved)?;
            Ok(moved)
        })?;
        made.extend(moved);
    }
    store.arm_chains(now)?;
    Ok(made)
}

/// B13, B15, B16: makes the next hop of `chain`, skipping a Door that cannot take it, or the
/// Landing when none is left.
fn advance(
    inner: &Inner,
    store: &mut Store,
    nodes: &[RailNode],
    now: i64,
    chain: &Chain,
    made: &mut Vec<Made>,
) -> Result<(), RpcError> {
    let mut rest = chain.rest.clone();
    let mut previous = chain.current;
    while !rest.is_empty() {
        let door = rest.remove(0);
        let Some(node) = nodes.iter().find(|node| node.id == door) else {
            continue;
        };
        if door == chain.origin.id
            || ended(node)
            || accept_receiver(inner, store, node).is_err()
            || store.count_open(&door)? >= OPEN_BOUND
        {
            continue;
        }
        let route = store
            .get_route(&chain.origin.id, &door)?
            .unwrap_or(Delivery::Auto);
        let (status, reason) = send_status(
            chain.origin.kind == ActorKind::User,
            route,
            store.is_takeover_active(&door),
        );
        let hop = store.insert(
            &chain.origin,
            &door,
            MessageKind::Question,
            &chain.body,
            None,
            status,
            reason,
            now,
            binding(node),
            Some(previous),
        )?;
        previous = hop.id;
        made.push(Made {
            message: hop.clone(),
            new: true,
        });
        if status != MessageStatus::Dropped {
            store.move_chain(chain.id, Some(hop.id), &rest)?;
            return Ok(());
        }
    }
    let landing = store.insert(
        &chain.origin,
        &contracts::Actor::user().id,
        MessageKind::Question,
        &chain.body,
        None,
        MessageStatus::Held,
        Some(Reason::Escalated),
        now,
        (0, 0),
        Some(previous),
    )?;
    made.push(Made {
        message: landing,
        new: true,
    });
    store.move_chain(chain.id, None, &[])
}

/// Whether `actor` is the Agent a hop was sent to.
pub(crate) fn is_hop_receiver(actor: &Actor, hop: &Message) -> bool {
    actor.kind == ActorKind::Agent && actor.id == hop.to
}
