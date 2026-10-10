//! B19, B20: what a Meta-agent is told about its children, composed from the Rail, the Todos, the
//! Pads and the Messages, which only the Daemon holds together. Nothing here reads a Terminal.

use std::collections::HashMap;
use std::sync::Mutex;

use contracts::agent::{Digest, DigestEntry, DigestItem, NodeKind, RailNode};
use contracts::message::Message;
use contracts::pad::Pad;
use contracts::todo::Todo;
use contracts::{Actor, ActorKind, Kind};
use rpc::RpcError;

const LAST_BYTES: usize = 120;
const PAD_NAME_BYTES: usize = 32;
const PADS: usize = 4;
const ENTRY_BYTES: usize = 512;
const CHILDREN: usize = 20;
const CUT: &str = "[cut]";

/// What the Daemon read to compose a digest.
pub struct Sources<'a> {
    pub nodes: &'a [RailNode],
    pub todos: &'a [Todo],
    pub pads: &'a [Pad],
    pub messages: &'a [Message],
}

/// What a Meta-agent was last told about a child, to see whether a change alters the entry (B21).
#[derive(Clone, PartialEq, Eq)]
struct Told {
    kind: Kind,
    todos: u32,
    /// The newest Pad write seen then, named or not.
    pads: i64,
    /// The entry as it was then, for the last push when the child leaves the Rail (B23).
    entry: DigestEntry,
}

#[derive(Default)]
struct Book {
    /// For each (Meta-agent, child), the newest `updated_at` among the child's Pads that an
    /// envelope already named, so the next envelope names only Pads written after it (B19).
    named: HashMap<(String, String), i64>,
    told: HashMap<(String, String), Told>,
}

/// An entry B21 decided to push. `changed` already recorded it as told, so a digest asked while
/// the Message is in flight does not repeat it; `release` takes that back if the send failed.
pub struct Push {
    key: (String, String),
    pub meta: String,
    pub entry: DigestEntry,
    told: Told,
    before: (Option<i64>, Option<Told>),
    /// The child left the Rail (B23): `told` is what `changed` took out of the book.
    gone: bool,
}

#[derive(Default)]
pub struct Seen(Mutex<Book>);

impl Seen {
    fn book(&self) -> std::sync::MutexGuard<'_, Book> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// B20: the digest of Meta-agent `id`, asked for by `caller`.
    pub fn ask(&self, caller: &Actor, id: &str, from: &Sources) -> Result<Digest, RpcError> {
        let allowed = match caller.kind {
            ActorKind::User => true,
            ActorKind::Agent => caller.id == id,
            ActorKind::Ext => false,
        };
        if !allowed {
            return Err(RpcError::forbidden(format!(
                "{} may not read the digest of agent {id}",
                caller.id
            )));
        }
        let node = from
            .nodes
            .iter()
            .find(|node| node.id == id && is_agent(node))
            .ok_or_else(|| RpcError::not_found(format!("agent {id}")))?;
        if node.kind != NodeKind::Workstream {
            return Err(RpcError::conflict(format!("agent {id} is no Meta-agent")));
        }
        let children = children_of(id, from);
        let mut book = self.book();
        let mut items: Vec<DigestItem> = Vec::new();
        for child in children.iter().take(CHILDREN) {
            let key = (id.to_owned(), child.id.clone());
            let after = book.named.get(&key).copied();
            let (entry, newest) = entry(child, id, from, after);
            if caller.kind == ActorKind::Agent {
                if let Some(newest) = newest {
                    book.named.insert(key.clone(), newest);
                }
                book.told.insert(key, told(&entry, child, from));
            }
            items.push(DigestItem::Child(entry));
        }
        if children.len() > CHILDREN {
            let more = u32::try_from(children.len() - CHILDREN).unwrap_or(u32::MAX);
            items.push(DigestItem::More { more });
        }
        Ok(Digest { children: items })
    }

    /// B21: what the Daemon records as told without sending it, so that only a change after now
    /// is pushed. Children that already have an entry keep it.
    pub fn baseline(&self, from: &Sources) {
        let mut book = self.book();
        for (meta, child) in pairs(from) {
            let key = (meta.id.clone(), child.id.clone());
            if !book.told.contains_key(&key) {
                let after = book.named.get(&key).copied();
                let (entry, _) = entry(child, &meta.id, from, after);
                let seen = told(&entry, child, from);
                book.told.insert(key, seen);
            }
        }
    }

    /// B21: the entries whose child's Kind, open Todo count or Pads changed since its Meta-agent
    /// was last told, oldest child first in Rail order. A change that leaves the entry as it was
    /// is none. Each one is recorded as told and its Pads as named before it is returned, under one
    /// lock, so no digest can name them again while the Message is sent.
    pub fn changed(&self, from: &Sources) -> Vec<Push> {
        let mut book = self.book();
        let mut pushes = Vec::new();
        for (meta, child) in pairs(from) {
            let key = (meta.id.clone(), child.id.clone());
            let after = book.named.get(&key).copied();
            let (entry, newest) = entry(child, &meta.id, from, after);
            let now = told(&entry, child, from);
            let moved = book.told.get(&key).is_none_or(|was| {
                was.kind != now.kind
                    || was.todos != now.todos
                    || (!entry.pads.is_empty() && now.pads > was.pads)
            });
            if moved {
                let before = (after, book.told.get(&key).cloned());
                if let Some(newest) = newest {
                    book.named.insert(key.clone(), newest);
                }
                book.told.insert(key.clone(), now.clone());
                pushes.push(Push {
                    key,
                    meta: meta.id.clone(),
                    entry,
                    told: now,
                    before,
                    gone: false,
                });
            }
        }
        pushes.extend(farewells(&mut book, from));
        pushes
    }

    /// B21: the Message for `push` was not sent, so its entry is as untold as it was, unless a
    /// digest asked since has told the Meta-agent something newer.
    pub fn release(&self, push: Push) {
        let mut book = self.book();
        if push.gone {
            book.told.entry(push.key).or_insert(push.told);
            return;
        }
        if book.told.get(&push.key) != Some(&push.told) {
            return;
        }
        match push.before.0 {
            Some(named) => book.named.insert(push.key.clone(), named),
            None => book.named.remove(&push.key),
        };
        match push.before.1 {
            Some(told) => book.told.insert(push.key, told),
            None => book.told.remove(&push.key),
        };
    }
}

/// B21: the body of a pushed digest.
#[must_use]
pub fn push_body(entry: &DigestEntry) -> String {
    format!(
        "[digest] {}",
        serde_json::to_string(entry).unwrap_or_default()
    )
}

fn told(entry: &DigestEntry, child: &RailNode, from: &Sources) -> Told {
    Told {
        kind: entry.kind,
        todos: entry.todos,
        pads: newest_pad(child, from),
        entry: entry.clone(),
    }
}

/// B23: one last push for each child told of that no longer has a node, to its Meta-agent if that
/// is still a Door, with the entry as it was last told; none when that already said the child
/// ended (`done`, `error`). A child that only moved to another Meta-agent has nothing to say.
fn farewells(book: &mut Book, from: &Sources) -> Vec<Push> {
    let gone: Vec<(String, String)> = book
        .told
        .keys()
        .filter(|(_, child)| {
            !from
                .nodes
                .iter()
                .any(|node| node.id == *child && is_agent(node))
        })
        .cloned()
        .collect();
    let mut pushes = Vec::new();
    for key in gone {
        let Some(told) = book.told.remove(&key) else {
            continue;
        };
        book.named.remove(&key);
        let door = from
            .nodes
            .iter()
            .any(|node| node.id == key.0 && node.kind == NodeKind::Workstream && is_agent(node));
        if door && !matches!(told.kind, Kind::Done | Kind::Error) {
            pushes.push(Push {
                meta: key.0.clone(),
                entry: told.entry.clone(),
                before: (None, Some(told.clone())),
                told,
                key,
                gone: true,
            });
        }
    }
    pushes
}

fn newest_pad(child: &RailNode, from: &Sources) -> i64 {
    from.pads
        .iter()
        .filter(|pad| pad.owner.kind == ActorKind::Agent && pad.owner.id == child.id)
        .map(|pad| pad.updated_at)
        .max()
        .unwrap_or(i64::MIN)
}

/// The direct children of Meta-agent `id` that have a Status, in Rail order.
fn children_of<'a>(id: &str, from: &'a Sources) -> Vec<&'a RailNode> {
    from.nodes
        .iter()
        .filter(|child| child.parent.as_deref() == Some(id) && is_agent(child))
        .filter(|child| child.status.is_some())
        .collect()
}

/// Every (Meta-agent, child) pair on the Rail, the Meta-agent being a Workstream whose Door ran.
fn pairs<'a>(from: &'a Sources) -> impl Iterator<Item = (&'a RailNode, &'a RailNode)> {
    from.nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Workstream && is_agent(node))
        .flat_map(|meta| {
            children_of(&meta.id, from)
                .into_iter()
                .map(move |child| (meta, child))
        })
}

/// An Agent, or a Workstream whose Door has run.
fn is_agent(node: &RailNode) -> bool {
    match node.kind {
        NodeKind::Agent => true,
        NodeKind::Workstream => node.attempt.is_some(),
        NodeKind::Terminal => false,
    }
}

/// B19: the entry for `child` as told to `meta`, and the newest Pad write it names. `after` is the
/// newest Pad write an earlier envelope named.
fn entry(
    child: &RailNode,
    meta: &str,
    from: &Sources,
    after: Option<i64>,
) -> (DigestEntry, Option<i64>) {
    let Some(status) = child.status.as_ref() else {
        unreachable!("the caller keeps only children that have a Status");
    };
    let told = from
        .messages
        .iter()
        .filter(|message| {
            message.from.kind == ActorKind::Agent
                && message.from.id == child.id
                && message.to == meta
        })
        .max_by_key(|message| message.id);
    let last = told.map_or(status.label.as_str(), |message| {
        message.body.lines().next().unwrap_or_default()
    });
    let todos = from
        .todos
        .iter()
        .filter(|todo| !todo.done && todo.home.as_deref() == Some(child.id.as_str()))
        .count();
    let mut written: Vec<&Pad> = from
        .pads
        .iter()
        .filter(|pad| pad.owner.kind == ActorKind::Agent && pad.owner.id == child.id)
        .filter(|pad| after.is_none_or(|after| pad.updated_at > after))
        .collect();
    written.sort_by_key(|pad| pad.updated_at);
    let newest = written.last().map(|pad| pad.updated_at);
    let mut pads: Vec<String> = Vec::new();
    if written.len() > PADS {
        pads.extend(
            written[written.len() - PADS..]
                .iter()
                .map(|pad| cut(&pad.name, PAD_NAME_BYTES)),
        );
        pads.push(CUT.to_owned());
    } else {
        pads.extend(written.iter().map(|pad| cut(&pad.name, PAD_NAME_BYTES)));
    }
    let entry = DigestEntry {
        name: child.name.clone(),
        kind: status.kind,
        last: cut(last, LAST_BYTES),
        todos: u32::try_from(todos).unwrap_or(u32::MAX),
        pads,
    };
    (fit(entry), newest)
}

/// `text` as is when it fits `max` bytes, else a prefix ending in `[cut]`, whole at most `max`.
pub(crate) fn cut(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max.saturating_sub(CUT.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{CUT}", &text[..end])
}

fn size(entry: &DigestEntry) -> usize {
    serde_json::to_string(entry).map_or(usize::MAX, |json| json.len())
}

/// B19: the whole entry is at most 512 bytes. Past that, `last` gives way first, then the Pad
/// names, then the name; every cut is marked.
fn fit(mut entry: DigestEntry) -> DigestEntry {
    while size(&entry) > ENTRY_BYTES && entry.last.len() > CUT.len() {
        let over = size(&entry) - ENTRY_BYTES;
        let max = entry.last.len().saturating_sub(over).max(CUT.len());
        entry.last = cut(&entry.last, max);
    }
    while size(&entry) > ENTRY_BYTES && entry.pads.len() > 1 {
        entry.pads.pop();
        if let Some(end) = entry.pads.last_mut() {
            *end = CUT.to_owned();
        }
    }
    while size(&entry) > ENTRY_BYTES && entry.name.len() > CUT.len() {
        let over = size(&entry) - ENTRY_BYTES;
        let max = entry.name.len().saturating_sub(over).max(CUT.len());
        entry.name = cut(&entry.name, max);
    }
    entry
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::agent::Worktree;
    use contracts::message::{MessageKind, MessageStatus};
    use contracts::{Kind, Status};

    fn status(kind: Kind, label: &str) -> Status {
        Status {
            kind,
            label: label.into(),
            since: 0,
        }
    }

    fn node(id: &str, kind: NodeKind, parent: Option<&str>, label: &str) -> RailNode {
        RailNode {
            id: id.into(),
            kind,
            name: format!("{id}-name"),
            parent: parent.map(Into::into),
            order: 0,
            status: (kind != NodeKind::Terminal).then(|| status(Kind::Working, label)),
            attempt: (kind != NodeKind::Terminal).then(|| "1".into()),
            status_revision: None,
            terminal_id: None,
            worktree: None::<Worktree>,
            can_resume: false,
            channel: None,
            work: None,
            stray: Vec::new(),
        }
    }

    fn todo(id: u32, home: Option<&str>, done: bool) -> Todo {
        Todo {
            id,
            title: format!("todo {id}"),
            body: String::new(),
            done,
            blockers: vec![],
            blocked: false,
            created_at: 0,
            creator: Actor::user(),
            home: home.map(Into::into),
        }
    }

    fn pad(name: &str, owner: &str, at: i64) -> Pad {
        Pad {
            name: name.into(),
            owner: Actor {
                kind: ActorKind::Agent,
                id: owner.into(),
                parent: None,
            },
            text: "ZEBRA".into(),
            updated_at: at,
        }
    }

    fn sent(id: u32, from: &str, to: &str, body: &str) -> Message {
        Message {
            id,
            from: Actor {
                kind: ActorKind::Agent,
                id: from.into(),
                parent: None,
            },
            to: to.into(),
            kind: MessageKind::Note,
            body: body.into(),
            reply_to: None,
            status: MessageStatus::Delivered,
            reason: None,
            passed_from: None,
            at: id.into(),
        }
    }

    fn rail() -> Vec<RailNode> {
        vec![
            node("m", NodeKind::Workstream, None, "coordinating"),
            node("a", NodeKind::Agent, Some("m"), "reading"),
            node("b", NodeKind::Agent, Some("m"), "testing"),
            node("t", NodeKind::Terminal, Some("m"), ""),
        ]
    }

    fn ask(
        seen: &Seen,
        nodes: &[RailNode],
        todos: &[Todo],
        pads: &[Pad],
        messages: &[Message],
    ) -> Vec<DigestItem> {
        let from = Sources {
            nodes,
            todos,
            pads,
            messages,
        };
        let m = Actor {
            kind: ActorKind::Agent,
            id: "m".into(),
            parent: None,
        };
        seen.ask(&m, "m", &from).unwrap().children
    }

    fn entries(items: &[DigestItem]) -> Vec<&DigestEntry> {
        items
            .iter()
            .filter_map(|item| match item {
                DigestItem::Child(entry) => Some(entry),
                DigestItem::More { .. } => None,
            })
            .collect()
    }

    #[test]
    fn b19_the_envelope_has_exactly_five_fields_and_leaves_out_terminal_output() {
        let items = ask(&Seen::default(), &rail(), &[], &[pad("notes", "a", 5)], &[]);
        let json = serde_json::to_value(&items[0]).unwrap();
        let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["kind", "last", "name", "pads", "todos"]);
        assert!(!json.to_string().contains("ZEBRA"));
        assert_eq!(items.len(), 2, "a Terminal child has no entry");
    }

    #[test]
    fn b19_last_is_the_first_line_of_the_newest_message_to_m_else_the_status_label() {
        let told = [
            sent(1, "a", "m", "old\nline"),
            sent(3, "a", "m", "newest first line\nsecond"),
            sent(2, "a", "other", "elsewhere"),
            sent(4, "b", "a", "not to m"),
        ];
        let items = ask(&Seen::default(), &rail(), &[], &[], &told);
        let items = entries(&items);
        assert_eq!(items[0].last, "newest first line");
        assert_eq!(items[1].last, "testing");
        assert_eq!(items[1].kind, Kind::Working);
    }

    #[test]
    fn b19_last_is_cut_at_120_bytes_ending_in_cut() {
        let long = "é".repeat(100);
        let items = ask(
            &Seen::default(),
            &rail(),
            &[],
            &[],
            &[sent(1, "a", "m", &long)],
        );
        let last = &entries(&items)[0].last;
        assert!(last.len() <= 120 && last.ends_with("[cut]"));
        assert!(long.starts_with(last.trim_end_matches("[cut]")));
    }

    #[test]
    fn b19_todos_counts_open_todos_homed_in_the_child() {
        let todos = [
            todo(1, Some("a"), false),
            todo(2, Some("a"), false),
            todo(3, Some("a"), true),
            todo(4, Some("b"), false),
            todo(5, None, false),
        ];
        let items = ask(&Seen::default(), &rail(), &todos, &[], &[]);
        let counts: Vec<_> = entries(&items).iter().map(|e| e.todos).collect();
        assert_eq!(counts, [2, 1]);
    }

    #[test]
    fn b19_pads_name_only_pads_written_since_the_previous_envelope() {
        let seen = Seen::default();
        let first = ask(
            &seen,
            &rail(),
            &[],
            &[pad("one", "a", 10), pad("x", "b", 11)],
            &[],
        );
        assert_eq!(entries(&first)[0].pads, ["one"]);
        let pads = [pad("one", "a", 10), pad("two", "a", 20), pad("x", "b", 11)];
        let second = ask(&seen, &rail(), &[], &pads, &[]);
        assert_eq!(entries(&second)[0].pads, ["two"]);
        assert!(entries(&second)[1].pads.is_empty());
    }

    #[test]
    fn b19_the_user_asking_does_not_hide_pads_from_m() {
        let seen = Seen::default();
        let pads = [pad("one", "a", 10)];
        let from = Sources {
            nodes: &rail(),
            todos: &[],
            pads: &pads,
            messages: &[],
        };
        let m = Actor {
            kind: ActorKind::Agent,
            id: "m".into(),
            parent: None,
        };
        let pads_of = |caller: &Actor| match &seen.ask(caller, "m", &from).unwrap().children[0] {
            DigestItem::Child(entry) => entry.pads.clone(),
            DigestItem::More { .. } => unreachable!(),
        };
        assert_eq!(pads_of(&Actor::user()), ["one"]);
        assert_eq!(pads_of(&Actor::user()), ["one"]);
        assert_eq!(pads_of(&m), ["one"]);
        assert!(pads_of(&m).is_empty());
    }

    #[test]
    fn b19_five_pads_name_the_newest_four_and_end_in_cut_and_a_long_name_is_cut_at_32() {
        let long = "n".repeat(40);
        let pads = [
            pad("p1", "a", 1),
            pad("p2", "a", 2),
            pad("p3", "a", 3),
            pad("p4", "a", 4),
            pad(&long, "a", 5),
        ];
        let items = ask(&Seen::default(), &rail(), &[], &pads, &[]);
        let names = &entries(&items)[0].pads;
        assert_eq!(names.len(), 5);
        assert_eq!(&names[..3], ["p2", "p3", "p4"]);
        assert!(names[3].len() == 32 && names[3].ends_with("[cut]"));
        assert_eq!(names[4], "[cut]");
    }

    #[test]
    fn b19_the_whole_entry_is_at_most_512_bytes() {
        let mut nodes = rail();
        nodes[1].name = "w".repeat(600);
        let quotes = "\"".repeat(200);
        let pads: Vec<_> = (0..6).map(|n| pad(&"\"".repeat(40), "a", n)).collect();
        let items = ask(
            &Seen::default(),
            &nodes,
            &[],
            &pads,
            &[sent(1, "a", "m", &quotes)],
        );
        for entry in entries(&items) {
            assert!(serde_json::to_string(entry).unwrap().len() <= 512);
        }
    }

    #[test]
    fn b20_children_are_in_rail_order_at_most_20_then_more() {
        let mut nodes = vec![node("m", NodeKind::Workstream, None, "x")];
        nodes.extend((0..23).map(|n| node(&format!("c{n}"), NodeKind::Agent, Some("m"), "x")));
        let items = ask(&Seen::default(), &nodes, &[], &[], &[]);
        assert_eq!(items.len(), 21);
        assert_eq!(entries(&items)[0].name, "c0-name");
        assert_eq!(entries(&items)[19].name, "c19-name");
        assert_eq!(items[20], DigestItem::More { more: 3 });
    }

    #[test]
    fn b20_forbidden_not_found_and_conflict() {
        let nodes = rail();
        let from = Sources {
            nodes: &nodes,
            todos: &[],
            pads: &[],
            messages: &[],
        };
        let seen = Seen::default();
        let agent = |id: &str| Actor {
            kind: ActorKind::Agent,
            id: id.into(),
            parent: None,
        };
        let code = |caller: &Actor, id: &str| seen.ask(caller, id, &from).unwrap_err().code;
        assert_eq!(code(&agent("a"), "m"), rpc::code::FORBIDDEN);
        assert_eq!(code(&Actor::daemon(), "m"), rpc::code::FORBIDDEN);
        assert_eq!(code(&Actor::user(), "nobody"), rpc::code::NOT_FOUND);
        assert_eq!(code(&Actor::user(), "t"), rpc::code::NOT_FOUND);
        assert_eq!(code(&Actor::user(), "a"), rpc::code::CONFLICT);
        assert!(seen.ask(&agent("m"), "m", &from).is_ok());
    }

    fn sources<'a>(
        nodes: &'a [RailNode],
        todos: &'a [Todo],
        pads: &'a [Pad],
        messages: &'a [Message],
    ) -> Sources<'a> {
        Sources {
            nodes,
            todos,
            pads,
            messages,
        }
    }

    #[test]
    fn b21_only_a_change_to_kind_todos_or_pads_is_pushed_and_once() {
        let seen = Seen::default();
        let mut nodes = rail();
        let none: Vec<Todo> = vec![];
        seen.baseline(&sources(&nodes, &none, &[], &[]));
        assert!(seen.changed(&sources(&nodes, &none, &[], &[])).is_empty());

        // A Kind change.
        nodes[1].status = Some(status(Kind::Idle, "done"));
        let pushes = seen.changed(&sources(&nodes, &none, &[], &[]));
        assert_eq!(pushes.len(), 1);
        assert_eq!(pushes[0].meta, "m");
        assert_eq!(pushes[0].entry.kind, Kind::Idle);
        assert!(push_body(&pushes[0].entry).starts_with("[digest] {"));
        assert!(seen.changed(&sources(&nodes, &none, &[], &[])).is_empty());

        // A label that leaves Kind, Todos and Pads as they were.
        nodes[1].status = Some(status(Kind::Idle, "still done"));
        assert!(seen.changed(&sources(&nodes, &none, &[], &[])).is_empty());

        // A Todo, then a Pad.
        let todos = vec![todo(1, Some("a"), false)];
        let pushes = seen.changed(&sources(&nodes, &todos, &[], &[]));
        assert_eq!(pushes[0].entry.todos, 1);
        let pads = vec![pad("notes", "a", 5)];
        let pushes = seen.changed(&sources(&nodes, &todos, &pads, &[]));
        assert_eq!(pushes[0].entry.pads, ["notes"]);
        assert!(
            seen.changed(&sources(&nodes, &todos, &pads, &[]))
                .is_empty()
        );
    }

    #[test]
    fn b21_a_push_is_told_at_once_and_offered_again_once_released() {
        let seen = Seen::default();
        let mut nodes = rail();
        let none: Vec<Todo> = vec![];
        seen.baseline(&sources(&nodes, &none, &[], &[]));
        nodes[1].status = Some(status(Kind::Idle, "done"));
        let mut pushes = seen.changed(&sources(&nodes, &none, &[], &[]));
        assert_eq!(pushes.len(), 1);
        assert!(seen.changed(&sources(&nodes, &none, &[], &[])).is_empty());
        seen.release(pushes.remove(0));
        assert_eq!(seen.changed(&sources(&nodes, &none, &[], &[])).len(), 1);
    }

    #[test]
    fn b21_pads_the_baseline_found_are_not_news_and_an_ask_by_m_tells_them() {
        let seen = Seen::default();
        let nodes = rail();
        let none: Vec<Todo> = vec![];
        let pads = vec![pad("old", "a", 5)];
        seen.baseline(&sources(&nodes, &none, &pads, &[]));
        assert!(seen.changed(&sources(&nodes, &none, &pads, &[])).is_empty());

        let pads = vec![pad("old", "a", 5), pad("new", "a", 9)];
        assert_eq!(seen.changed(&sources(&nodes, &none, &pads, &[])).len(), 1);
        ask(&seen, &nodes, &none, &pads, &[]);
        assert!(seen.changed(&sources(&nodes, &none, &pads, &[])).is_empty());
    }

    #[test]
    fn b23_a_removed_child_gets_one_last_push_with_its_kind_and_a_gone_door_none() {
        let seen = Seen::default();
        let none: Vec<Todo> = vec![];
        let mut nodes = rail();
        seen.baseline(&sources(&nodes, &none, &[], &[]));

        nodes.remove(1);
        let pushes = seen.changed(&sources(&nodes, &none, &[], &[]));
        assert_eq!(pushes.len(), 1);
        assert_eq!(
            (pushes[0].meta.as_str(), pushes[0].entry.name.as_str()),
            ("m", "a-name")
        );
        assert_eq!(pushes[0].entry.kind, Kind::Working);
        assert!(seen.changed(&sources(&nodes, &none, &[], &[])).is_empty());

        // The Door goes with its child: nobody is left to tell.
        nodes.retain(|node| node.id == "t");
        assert!(seen.changed(&sources(&nodes, &none, &[], &[])).is_empty());
    }

    #[test]
    fn b23_a_child_that_ended_before_it_was_removed_and_one_that_moved_get_no_second_push() {
        let seen = Seen::default();
        let none: Vec<Todo> = vec![];
        let mut nodes = rail();
        nodes.push(node("n", NodeKind::Workstream, None, "other"));
        seen.baseline(&sources(&nodes, &none, &[], &[]));
        nodes[1].status = Some(status(Kind::Done, "finished"));
        assert_eq!(seen.changed(&sources(&nodes, &none, &[], &[])).len(), 1);

        nodes.remove(1);
        let pushes = seen.changed(&sources(&nodes, &none, &[], &[]));
        assert!(
            pushes.is_empty(),
            "the ended child was told of when it ended"
        );

        // `b` moves under `n`: `n` is told of its new child, and `m` is told of no removal.
        nodes[1].parent = Some("n".into());
        let pushes = seen.changed(&sources(&nodes, &none, &[], &[]));
        assert_eq!(
            pushes.iter().map(|p| p.meta.as_str()).collect::<Vec<_>>(),
            ["n"]
        );
    }

    #[test]
    fn b23_a_last_push_that_failed_is_offered_again() {
        let seen = Seen::default();
        let none: Vec<Todo> = vec![];
        let mut nodes = rail();
        seen.baseline(&sources(&nodes, &none, &[], &[]));
        nodes.remove(1);
        let mut pushes = seen.changed(&sources(&nodes, &none, &[], &[]));
        seen.release(pushes.remove(0));
        assert_eq!(seen.changed(&sources(&nodes, &none, &[], &[])).len(), 1);
    }
}
