//! B22: what keeps pushed digests (B21) from flooding a Meta-agent. At most 8 go out in any
//! 60 000 ms; the rest wait and leave as one rollup; at most 20 stay `pending` or `held`, the
//! oldest two merging into one when a twenty-first arrives. No cap loses a count: each digest has
//! a weight, read from its body, and a rollup or a merge weighs what it holds.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use contracts::Actor;
use contracts::agent::DigestEntry;
use contracts::message::{Message, MessageKind, SendParams};
use rpc::{Ctx, RpcError, code};

use crate::digest::{cut, push_body};
use crate::{Inner, Messages, lock};

pub(crate) const PREFIX: &str = "[digest]";
const WINDOW_MS: i64 = 60_000;
const PUSHES: usize = 8;
const OPEN_DIGESTS: usize = 20;
const ROLLUP_CHILDREN: usize = 8;
const ROLLUP_BYTES: usize = 4096;
const MORE_BYTES: usize = 512;

/// What was sent to each Meta-agent lately, and the changes still waiting for the window.
#[derive(Default)]
pub(crate) struct Ledger {
    metas: HashMap<String, Slot>,
}

#[derive(Default)]
struct Slot {
    /// When the pushes of the last window went out, oldest first.
    sent: VecDeque<i64>,
    /// The entries of the changes held back, oldest first.
    waiting: Vec<DigestEntry>,
}

impl Slot {
    fn expire(&mut self, now: i64) {
        while self.sent.front().is_some_and(|at| *at <= now - WINDOW_MS) {
            self.sent.pop_front();
        }
    }
}

/// Whether a Message from `from` with `body` is a pushed digest.
pub(crate) fn is_digest(from: &Actor, body: &str) -> bool {
    *from == Actor::daemon() && body.starts_with(PREFIX)
}

/// B22(c): the changes a pushed digest stands for: 1 for a plain push, K for a rollup, N for a
/// merge.
pub(crate) fn weight(body: &str) -> u32 {
    let head = body.lines().next().unwrap_or_default();
    let Some(counted) = head.strip_prefix("[digest] [") else {
        return 1;
    };
    let digits = counted.bytes().take_while(u8::is_ascii_digit).count();
    let rest = &counted[digits..];
    if rest.starts_with(" updates held back]") || rest.starts_with(" earlier updates: ") {
        counted[..digits].parse().unwrap_or(1)
    } else {
        1
    }
}

/// The entries a pushed digest holds, in the order it lists them.
fn entries_of(body: &str) -> Vec<DigestEntry> {
    body.lines()
        .map(|line| line.strip_prefix("[digest] ").unwrap_or(line))
        .filter(|line| line.starts_with('{'))
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// `header`, then `entries` (newest first, one per child) one to a line, at most 8 and 4096 bytes
/// in all; the children left out are named on one last line, cut at 512 bytes.
fn compose(header: &str, entries: &[DigestEntry]) -> String {
    let lines: Vec<String> = entries
        .iter()
        .map(|entry| serde_json::to_string(entry).unwrap_or_default())
        .collect();
    let fit = |reserve: usize| {
        let mut used = header.len();
        let mut taken = 0;
        for line in &lines {
            if taken == ROLLUP_CHILDREN || used + 1 + line.len() + reserve > ROLLUP_BYTES {
                break;
            }
            used += 1 + line.len();
            taken += 1;
        }
        taken
    };
    let mut taken = fit(0);
    if taken < lines.len() {
        taken = fit(1 + MORE_BYTES);
    }
    let mut body = header.to_owned();
    for line in &lines[..taken] {
        body.push('\n');
        body.push_str(line);
    }
    if taken < lines.len() {
        let names: Vec<&str> = entries[taken..]
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        let more = format!("[+{} more: {}]", names.len(), names.join(", "));
        body.push('\n');
        body.push_str(&cut(&more, MORE_BYTES));
    }
    body
}

/// The newest entry of each child, from `entries` newest first.
fn newest_per_child(entries: impl IntoIterator<Item = DigestEntry>) -> Vec<DigestEntry> {
    let mut seen = std::collections::HashSet::new();
    entries
        .into_iter()
        .filter(|entry| seen.insert(entry.name.clone()))
        .collect()
}

/// B22(b): the Message that stands for `older` and `newer`.
pub(crate) fn merged_body(older: &Message, newer: &Message) -> String {
    let count = weight(&older.body) + weight(&newer.body);
    let header = format!(
        "{PREFIX} [{count} earlier updates: #{} #{}]",
        older.id, newer.id
    );
    let entries = newest_per_child(
        entries_of(&newer.body)
            .into_iter()
            .chain(entries_of(&older.body)),
    );
    compose(&header, &entries)
}

/// B22(a): the Message that stands for the `waiting` changes, oldest first.
fn rollup_body(waiting: &[DigestEntry]) -> String {
    let header = format!("{PREFIX} [{} updates held back]", waiting.len());
    compose(&header, &newest_per_child(waiting.iter().rev().cloned()))
}

/// B21, B22: sends `meta` the digest of `entry` now, or holds it back for the window's rollup.
pub(crate) async fn push(
    inner: &Arc<Inner>,
    meta: &str,
    entry: &DigestEntry,
) -> Result<(), RpcError> {
    let mut ledger = inner.pushes.lock().await;
    let now = (inner.clock)();
    let slot = ledger.metas.entry(meta.to_owned()).or_default();
    slot.expire(now);
    if slot.waiting.is_empty() && slot.sent.len() < PUSHES {
        send_digest(inner, meta, push_body(entry)).await?;
        slot.sent.push_back(now);
        return Ok(());
    }
    slot.waiting.push(entry.clone());
    roll_up(inner, meta, slot, now).await;
    Ok(())
}

/// B22(a): at the window's end, the changes held back go out as one Message.
pub(crate) async fn flush(inner: &Arc<Inner>) {
    let mut ledger = inner.pushes.lock().await;
    let now = (inner.clock)();
    let metas: Vec<String> = ledger.metas.keys().cloned().collect();
    for meta in metas {
        if let Some(slot) = ledger.metas.get_mut(&meta) {
            slot.expire(now);
            roll_up(inner, &meta, slot, now).await;
        }
    }
    ledger
        .metas
        .retain(|_, slot| !slot.sent.is_empty() || !slot.waiting.is_empty());
}

async fn roll_up(inner: &Arc<Inner>, meta: &str, slot: &mut Slot, now: i64) {
    if slot.waiting.is_empty() || slot.sent.len() >= PUSHES {
        return;
    }
    match send_digest(inner, meta, rollup_body(&slot.waiting)).await {
        Ok(()) => {
            slot.sent.push_back(now);
            slot.waiting.clear();
        }
        // A Meta-agent that ended or left the Rail has no use for them (B9, B23).
        Err(err) if matches!(err.code, code::CONFLICT | code::NOT_FOUND) => slot.waiting.clear(),
        Err(err) => crate::log_failure("push", &err.message),
    }
}

/// Sends one pushed digest, then merges the oldest two while more than 20 are open.
async fn send_digest(inner: &Arc<Inner>, meta: &str, body: String) -> Result<(), RpcError> {
    let messages = Messages {
        inner: Arc::clone(inner),
    };
    let ctx = Ctx {
        actor: Actor::daemon(),
        bus: inner.bus.clone(),
        touches: Arc::clone(&inner.touches),
    };
    let params = |body: String| SendParams {
        to: meta.to_owned(),
        kind: MessageKind::Note,
        body,
        reply_to: None,
    };
    messages.send_message(&ctx, params(body), false).await?;
    while lock(&inner.store).open_digests(meta)?.len() > OPEN_DIGESTS {
        if let Err(err) = messages
            .send_message(&ctx, params(PREFIX.to_owned()), true)
            .await
        {
            crate::log_failure("push", &err.message);
            break;
        }
    }
    Ok(())
}
