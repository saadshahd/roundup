//! Every RPC method with its parameter and result types, as TypeScript expressions.
//! Aliases are `<module>_<Type>` and resolve to `contracts/generated/<module>/<Type>.ts`
//! (or `generated/<Type>.ts` for the `common` module).

pub struct Method {
    pub name: &'static str,
    pub params: &'static str,
    pub result: &'static str,
}

const fn m(name: &'static str, params: &'static str, result: &'static str) -> Method {
    Method {
        name,
        params,
        result,
    }
}

pub const METHODS: &[Method] = &[
    m("daemon.ping", "null", "{ pong: true }"),
    m("daemon.identify", "common_IdentifyParams", "null"),
    m("events.subscribe", "null", "null"),
    m(
        "provenance.history",
        "common_HistoryParams",
        "common_Touch[]",
    ),
    m(
        "provenance.touched",
        "common_TouchedParams",
        "common_Touch[]",
    ),
    m(
        "terminal.spawn",
        "terminal_SpawnParams",
        "terminal_TerminalId",
    ),
    m("terminal.write", "terminal_WriteParams", "null"),
    m("terminal.resize", "terminal_ResizeParams", "null"),
    m("terminal.kill", "terminal_TerminalId", "null"),
    m("terminal.list", "null", "terminal_TerminalInfo[]"),
    m("todo.create", "todo_CreateParams", "todo_Todo"),
    m("todo.get", "todo_TodoId", "todo_Todo"),
    m("todo.list", "null", "todo_Todo[]"),
    m("todo.update", "todo_UpdateParams", "todo_Todo"),
    m("todo.complete", "todo_TodoId", "todo_Todo"),
    m("todo.setBlockers", "todo_SetBlockersParams", "todo_Todo"),
    m("todo.delete", "todo_TodoId", "null"),
    m("pad.create", "pad_CreateParams", "pad_Pad"),
    m("pad.read", "pad_PadName", "pad_Pad"),
    m("pad.list", "null", "pad_Pad[]"),
    m("pad.write", "pad_WriteParams", "pad_Pad"),
    m("pad.append", "pad_AppendParams", "pad_Pad"),
    m("pad.setOwner", "pad_SetOwnerParams", "pad_Pad"),
    m("pad.delete", "pad_PadName", "null"),
    m("pad.export", "pad_ExportParams", "null"),
    m("pad.setStorage", "pad_SetStorageParams", "null"),
    m("agent.spawn", "agent_SpawnParams", "agent_RailNode"),
    m("agent.stop", "agent_NodeId", "null"),
    m("agent.signal", "agent_SignalParams", "null"),
    m("rail.tree", "null", "agent_RailNode[]"),
    m(
        "rail.createGroup",
        "agent_CreateGroupParams",
        "agent_RailNode",
    ),
    m("rail.move", "agent_MoveParams", "null"),
    m("rail.rename", "agent_RenameParams", "agent_RailNode"),
    m("rail.promote", "agent_NodeId", "agent_RailNode"),
    m(
        "rail.spawnTerminal",
        "agent_SpawnTerminalParams",
        "agent_RailNode",
    ),
    m("project.setWorktrees", "project_Worktrees", "null"),
    m("project.get", "null", "project_ProjectSettings"),
];

/// Render `METHODS` as `contracts/generated/methods.ts`.
pub fn render_typescript() -> String {
    let mut aliases = std::collections::BTreeSet::new();
    for method in METHODS {
        for expr in [method.params, method.result] {
            for token in expr.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
                if let Some((module, ty)) = token.split_once('_')
                    && ty.chars().next().is_some_and(char::is_uppercase)
                {
                    aliases.insert((module, ty));
                }
            }
        }
    }
    let mut out = String::from("// Generated from crates/contracts/src/methods.rs. Do not edit.\n");
    for (module, ty) in &aliases {
        let path = if *module == "common" {
            (*ty).to_string()
        } else {
            format!("{module}/{ty}")
        };
        out.push_str(&format!(
            "import type {{ {ty} as {module}_{ty} }} from \"./{path}\";\n"
        ));
    }
    out.push_str("\nexport type RpcMethods = {\n");
    for method in METHODS {
        out.push_str(&format!(
            "  \"{}\": {{ params: {}; result: {} }};\n",
            method.name, method.params, method.result
        ));
    }
    out.push_str("};\n\nexport type RpcMethodName = keyof RpcMethods;\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_are_unique() {
        let mut names: Vec<_> = METHODS.iter().map(|m| m.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), METHODS.len());
    }

    #[test]
    fn writes_methods_ts() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../contracts/generated/methods.ts"
        );
        std::fs::write(path, render_typescript()).unwrap();
    }
}
