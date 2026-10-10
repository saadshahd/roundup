//! E2, E3: what an Agent is told about where it stands. The Daemon composes it here from the Rail
//! and the Todos, which only it holds together; `agents` owns the vendor's words for the text.

use contracts::agent::{
    Brief, Context, ContextAsk, ContextParent, ContextPeer, ContextSelf, ContextTodo, NodeKind,
    ParentNode, RailNode,
};
use contracts::todo::Todo;
use contracts::{Actor, ActorKind, Kind};
use rpc::RpcError;

/// An Agent, or a Room whose Door has run: only these have a Context.
fn is_agent(node: &RailNode) -> bool {
    match node.kind {
        NodeKind::Agent => true,
        NodeKind::Room => node.attempt.is_some(),
        NodeKind::Terminal => false,
    }
}

/// E2: the Context of Agent `id`. `nodes` is `rail.tree`, in Rail order.
pub(crate) fn compose(nodes: &[RailNode], todos: &[Todo], id: &str) -> Result<Context, RpcError> {
    let node = nodes
        .iter()
        .find(|node| node.id == id && is_agent(node))
        .ok_or_else(|| RpcError::not_found(format!("agent {id}")))?;
    let status = |node: &RailNode| {
        node.status
            .clone()
            .ok_or_else(|| RpcError::internal(format!("agent {} has no Status", node.id)))
    };
    let parent = node
        .parent
        .as_deref()
        .and_then(|parent| nodes.iter().find(|candidate| candidate.id == parent));
    let ask = match parent {
        Some(parent) if is_agent(parent) => parent.id.clone(),
        _ => Actor::user().id,
    };
    let peers = nodes
        .iter()
        .filter(|peer| peer.parent == node.parent && peer.id != node.id && is_agent(peer))
        .map(|peer| {
            Ok(ContextPeer {
                id: peer.id.clone(),
                name: peer.name.clone(),
                status: status(peer)?,
            })
        })
        .collect::<Result<_, RpcError>>()?;
    let todos = todos
        .iter()
        .filter(|todo| {
            !todo.done && todo.creator.kind == ActorKind::Agent && todo.creator.id == node.id
        })
        .map(|todo| ContextTodo {
            id: todo.id,
            title: todo.title.clone(),
            blocked: todo.blocked,
        })
        .collect();
    Ok(Context {
        this: ContextSelf {
            id: node.id.clone(),
            name: node.name.clone(),
            status: status(node)?,
        },
        parent: parent.map(|parent| ContextParent {
            id: parent.id.clone(),
            name: parent.name.clone(),
            node: if is_agent(parent) {
                ParentNode::MetaAgent
            } else {
                ParentNode::Group
            },
        }),
        ask: ContextAsk { to: ask },
        peers,
        todos,
    })
}

fn kind_word(kind: Kind) -> &'static str {
    match kind {
        Kind::Error => "error",
        Kind::NeedsYou => "needs-you",
        Kind::Blocked => "blocked",
        Kind::Working => "working",
        Kind::Idle => "idle",
        Kind::Done => "done",
    }
}

/// E3: `context` in words, one line each for the parent, whom to ask, every peer and every Todo.
fn words(context: &Context) -> String {
    let mut lines = vec![match &context.parent {
        Some(parent) => {
            let what = match parent.node {
                ParentNode::MetaAgent => "a Room with a Door",
                ParentNode::Group => "a Room",
            };
            format!("Parent: {} (id {}), {what}.", parent.name, parent.id)
        }
        None => "Parent: none, you are at the Project root.".to_owned(),
    }];
    lines.push(if context.ask.to == Actor::user().id {
        format!(
            "Ask: the user, with message_send to \"{}\".",
            context.ask.to
        )
    } else {
        format!(
            "Ask: your Door, with message_send to \"{}\".",
            context.ask.to
        )
    });
    if context.peers.is_empty() {
        lines.push("Peers: none.".to_owned());
    }
    for peer in &context.peers {
        lines.push(format!(
            "Peer: {} (id {}), {}.",
            peer.name,
            peer.id,
            kind_word(peer.status.kind)
        ));
    }
    if context.todos.is_empty() {
        lines.push("Todos: none open.".to_owned());
    }
    for todo in &context.todos {
        let blocked = if todo.blocked { " (blocked)" } else { "" };
        lines.push(format!("Todo {}: {}{blocked}.", todo.id, todo.title));
    }
    lines.join("\n")
}

/// E3: what `rup context` prints for `context`'s Agent, in the vendor's own shape.
pub(crate) fn brief(context: &Context) -> Brief {
    Brief {
        stdout: agents::claude_code::session_start_context(&words(context)),
    }
}

#[cfg(test)]
mod tests {
    use contracts::Status;
    use serde_json::json;

    use super::*;

    fn status(kind: Kind) -> Option<Status> {
        Some(Status {
            kind,
            label: "x".into(),
            since: 1,
        })
    }

    fn node(id: &str, kind: NodeKind, name: &str, parent: Option<&str>, door: bool) -> RailNode {
        RailNode {
            id: id.into(),
            kind,
            name: name.into(),
            parent: parent.map(str::to_owned),
            order: 0,
            status: (kind != NodeKind::Terminal)
                .then(|| status(Kind::Idle))
                .flatten(),
            attempt: (kind == NodeKind::Agent || door).then(|| "1".into()),
            status_revision: None,
            terminal_id: None,
            worktree: None,
            can_resume: false,
            channel: None,
        }
    }

    fn todo(id: u32, creator: &str, done: bool, blockers: Vec<u32>, blocked: bool) -> Todo {
        Todo {
            id,
            title: format!("todo {id}"),
            body: String::new(),
            done,
            blockers,
            blocked,
            created_at: 0,
            creator: Actor {
                kind: ActorKind::Agent,
                id: creator.into(),
                parent: None,
            },
            home: None,
        }
    }

    /// Meta-agent `m` (a Room with a Door) holding `a` and `b`; plain Group `g` holding `c`; a
    /// sibling Door `n` beside `m`.
    fn rail() -> Vec<RailNode> {
        vec![
            node("1", NodeKind::Room, "m", None, true),
            node("2", NodeKind::Agent, "a", Some("1"), false),
            node("3", NodeKind::Agent, "b", Some("1"), false),
            node("4", NodeKind::Terminal, "zsh", Some("1"), false),
            node("5", NodeKind::Room, "g", None, false),
            node("6", NodeKind::Agent, "c", Some("5"), false),
            node("7", NodeKind::Room, "n", None, true),
        ]
    }

    #[test]
    fn e2_an_agent_under_a_door_asks_the_door_and_has_its_siblings_as_peers() {
        let context = compose(&rail(), &[], "2").unwrap();

        assert_eq!(
            serde_json::to_value(&context).unwrap(),
            json!({
                "self": {"id": "2", "name": "a", "status": {"kind": "idle", "label": "x", "since": 1}},
                "parent": {"id": "1", "name": "m", "node": "meta-agent"},
                "ask": {"to": "1"},
                "peers": [{"id": "3", "name": "b", "status": {"kind": "idle", "label": "x", "since": 1}}],
                "todos": [],
            })
        );
    }

    #[test]
    fn e2_an_agent_in_a_plain_group_asks_the_user_and_has_no_peers() {
        let context = compose(&rail(), &[], "6").unwrap();

        assert_eq!(context.ask.to, "you");
        assert!(context.peers.is_empty());
        let parent = context.parent.unwrap();
        assert_eq!((parent.id.as_str(), parent.node), ("5", ParentNode::Group));
    }

    #[test]
    fn e2_an_agent_at_the_project_root_has_no_parent_and_asks_the_user() {
        let mut nodes = rail();
        nodes.push(node("8", NodeKind::Agent, "r", None, false));

        let context = compose(&nodes, &[], "8").unwrap();

        assert_eq!(context.parent, None);
        assert_eq!(context.ask.to, "you");
        // A sibling Door at the root is a peer; a plain Group and a Terminal are not.
        let peers: Vec<_> = context.peers.iter().map(|peer| peer.id.as_str()).collect();
        assert_eq!(peers, ["1", "7"]);
    }

    #[test]
    fn e2_the_todos_are_the_open_ones_the_agent_created() {
        let todos = [
            todo(1, "2", false, vec![], false),
            todo(2, "2", true, vec![], false),
            todo(3, "3", false, vec![], false),
            todo(4, "2", false, vec![1], true),
        ];

        let context = compose(&rail(), &todos, "2").unwrap();

        let seen: Vec<_> = context
            .todos
            .iter()
            .map(|todo| (todo.id, todo.blocked))
            .collect();
        assert_eq!(seen, [(1, false), (4, true)]);
    }

    #[test]
    fn e2_a_terminal_a_plain_group_and_a_missing_id_are_not_found() {
        for id in ["4", "5", "99"] {
            let err = compose(&rail(), &[], id).unwrap_err();
            assert_eq!(err.code, rpc::code::NOT_FOUND, "{id}");
        }
    }

    #[test]
    fn e3_the_brief_is_the_vendors_shape_around_one_line_each() {
        let todos = [
            todo(1, "2", false, vec![], false),
            todo(4, "2", false, vec![1], true),
        ];
        let context = compose(&rail(), &todos, "2").unwrap();

        let stdout = brief(&context).stdout;

        let shaped: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(
            shaped["hookSpecificOutput"]["hookEventName"],
            "SessionStart"
        );
        assert_eq!(
            shaped["hookSpecificOutput"]["additionalContext"],
            "Parent: m (id 1), a Room with a Door.\n\
             Ask: your Door, with message_send to \"1\".\n\
             Peer: b (id 3), idle.\n\
             Todo 1: todo 1.\n\
             Todo 4: todo 4 (blocked)."
        );
    }

    #[test]
    fn e3_the_brief_of_a_lone_agent_says_so_in_words() {
        let context = compose(&rail(), &[], "6").unwrap();

        let text = words(&context);

        assert_eq!(
            text,
            "Parent: g (id 5), a Room.\n\
             Ask: the user, with message_send to \"you\".\n\
             Peers: none.\n\
             Todos: none open."
        );
    }
}
