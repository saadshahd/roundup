//! E1: the Brief, the text file Claude Code appends to its system prompt. It holds only what never
//! changes for an Agent id: who it is, that roundup supervises it, the tools `rup mcp` offers and,
//! for a Door, B24's role guidance. A name, Terminal id, parent, peer or Todo changes, so none is
//! written here. A tool is described only once `crates/rup` offers it: landing `agent_context`,
//! `message_send`, `ask_user` or `agent_spawn` updates this list and its test together.

/// What an Agent is to roundup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Any Agent started by `agent.spawn`.
    Agent,
    /// A Room's Door, started by `rail.startDoor`.
    Door,
}

/// The tools `rup mcp` offers, with when to use each.
const TOOLS: [(&str, &str); 13] = [
    ("todo_create", "record a piece of work that is not done yet"),
    ("todo_get", "read one Todo before changing it"),
    ("todo_list", "see all the work already recorded"),
    (
        "todo_update",
        "change a Todo's title or body as the work becomes clearer",
    ),
    ("todo_complete", "mark a Todo done once its outcome is real"),
    (
        "todo_setBlockers",
        "replace the Todos that must finish before this one",
    ),
    (
        "pad_create",
        "start a Pad, a named text you own, for notes you want to keep",
    ),
    ("pad_read", "read one Pad"),
    ("pad_list", "see which Pads exist"),
    ("pad_write", "rewrite a Pad you own"),
    ("pad_append", "add text to the end of any Pad"),
    ("pad_setOwner", "give a Pad to another Actor"),
    ("pad_delete", "delete a Pad you no longer need"),
];

const DOOR: &str = "\
## Your role

You are this Room's coordinating Door. Use the Todo tools to record the work and update it as it changes. \
Report the outcome to the user when the work ends. \
When the user's goal or a taste limit is unclear, ask in this Terminal and wait for the answer. \
You cannot start other Agents, message them, or put a structured question to the user: \
do not claim or promise any of that.
";

/// The Brief text for Agent `id` in `role`: the same text every time for the same two.
pub fn text(id: u64, role: Role) -> String {
    let mut text = format!(
        "# roundup\n\n\
         Your Agent id is {id}. It never changes. roundup started you and supervises you; \
         the user can see your Status and read your Terminal.\n\n\
         ## Tools\n\n\
         The `roundup` MCP server offers these tools:\n\n"
    );
    for (name, when) in TOOLS {
        text.push_str(&format!("- `{name}`: {when}.\n"));
    }
    if role == Role::Door {
        text.push('\n');
        text.push_str(DOOR);
    }
    text
}
