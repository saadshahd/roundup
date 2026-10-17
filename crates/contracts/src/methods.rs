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
    m("ask.send", "ask_SendParams", "ask_Ask"),
    m("ask.list", "null", "ask_Ask[]"),
    m("ask.pick", "ask_PickParams", "ask_Ask"),
    m(
        "daemon.ping",
        "null",
        "{ pong: true, pid: number, project: string, contract: { fingerprint: string, methods: string[] } }",
    ),
    m("daemon.identify", "common_IdentifyParams", "null"),
    m(
        "events.subscribe",
        "common_SubscribeParams | null",
        "common_SubscribeResult",
    ),
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
    m("terminal.open", "terminal_OpenParams", "terminal_Opened"),
    m(
        "terminal.close",
        "terminal_TerminalId",
        "null | agent_HeldAction",
    ),
    m(
        "terminal.write",
        "terminal_WriteParams",
        "null | agent_HeldAction",
    ),
    m(
        "terminal.resize",
        "terminal_ResizeParams",
        "terminal_ResizeAction | null",
    ),
    m(
        "terminal.resizeHistory",
        "terminal_TerminalId",
        "terminal_ResizeAction[]",
    ),
    m(
        "terminal.kill",
        "terminal_TerminalId",
        "null | agent_HeldAction",
    ),
    m("terminal.show", "terminal_TerminalId", "null"),
    m("terminal.list", "null", "terminal_TerminalInfo[]"),
    m(
        "terminal.snapshot",
        "terminal_TerminalId",
        "terminal_Snapshot",
    ),
    m(
        "terminal.output",
        "terminal_TerminalId",
        "terminal_OutputStarted",
    ),
    m("terminal.outputStop", "terminal_TerminalId", "null"),
    m("terminal.screen", "terminal_TerminalId", "terminal_Screen"),
    m("terminal.read", "terminal_ReadParams", "terminal_Read"),
    m(
        "terminal.search",
        "terminal_SearchParams",
        "terminal_Search",
    ),
    m("todo.create", "todo_CreateParams", "todo_Todo"),
    m("todo.get", "todo_TodoId", "todo_Todo"),
    m("todo.getHistory", "todo_TodoId", "todo_History"),
    m("todo.readHistory", "todo_ReadHistoryParams", "null"),
    m("todo.list", "null", "todo_Todo[]"),
    m("todo.update", "todo_UpdateParams", "todo_Todo"),
    m("todo.complete", "todo_TodoId", "todo_Todo"),
    m("todo.setBlockers", "todo_SetBlockersParams", "todo_Todo"),
    m("todo.move", "todo_MoveParams", "todo_Todo"),
    m("todo.reorder", "todo_ReorderParams", "todo_Todo"),
    m("todo.delete", "todo_TodoId", "null"),
    m("pad.create", "pad_CreateParams", "pad_Pad"),
    m("pad.read", "pad_PadName", "pad_Pad"),
    m("pad.list", "null", "pad_Pad[]"),
    m("pad.write", "pad_WriteParams", "pad_Pad"),
    m("pad.append", "pad_AppendParams", "pad_Pad"),
    m("pad.appendSection", "pad_AppendSectionParams", "pad_Pad"),
    m("pad.setOwner", "pad_SetOwnerParams", "pad_Pad"),
    m("pad.move", "pad_MoveParams", "pad_Pad"),
    m("pad.delete", "pad_PadName", "null"),
    m("pad.export", "pad_ExportParams", "null"),
    m("pad.setStorage", "pad_SetStorageParams", "null"),
    m("agent.spawn", "agent_SpawnParams", "agent_RailNode"),
    m("agent.extras", "null", "agent_Extras"),
    m(
        "agent.extras_add",
        "agent_ExtrasAddParams",
        "agent_RailNode",
    ),
    m("agent.actionHistory", "agent_NodeId", "agent_HeldAction[]"),
    m("agent.stop", "agent_NodeId", "null | agent_HeldAction"),
    m(
        "agent.prompt",
        "agent_PromptParams",
        "null | agent_HeldAction",
    ),
    m("agent.interrupt", "agent_NodeId", "null | agent_HeldAction"),
    m("agent.worktreeState", "agent_NodeId", "agent_WorktreeState"),
    m("agent.land", "agent_NodeId", "agent_Landed"),
    m("agent.discard", "agent_NodeId", "null"),
    m("agent.context", "agent_NodeId", "agent_Context"),
    m("agent.brief", "agent_NodeId", "agent_Brief"),
    m("agent.skills", "agent_NodeId", "agent_Skills"),
    m("agent.digest", "agent_NodeId", "agent_Digest"),
    m("agent.wake", "agent_WakeParams", "agent_Woken"),
    m(
        "agent.spawnPrompt",
        "agent_NodeId",
        "{ prompt: string | null }",
    ),
    m(
        "agent.setOrder",
        "agent_SetOrderParams",
        "agent_RailNode | agent_HeldAction",
    ),
    m("agent.channelUp", "agent_NodeId", "null"),
    m(
        "agent.signal",
        "agent_SignalParams",
        "agent_SignalOutcome | null",
    ),
    m(
        "agent.permission",
        "decision_PermissionParams",
        "decision_PermissionOutput",
    ),
    m("agent.ask", "decision_AskCall", "decision_AskOutput"),
    m("decision.list", "null", "decision_Decision[]"),
    m(
        "decision.answers",
        "decision_AnswersParams",
        "decision_AnsweredDecision[]",
    ),
    m(
        "decision.capture",
        "decision_CaptureParams",
        "decision_PromptIdentity",
    ),
    m(
        "decision.answer",
        "decision_AnswerParams",
        "null | agent_HeldAction",
    ),
    m("rail.tree", "null", "agent_RailNode[]"),
    m(
        "rail.createWorkstream",
        "agent_CreateWorkstreamParams",
        "agent_RailNode",
    ),
    m("rail.move", "agent_MoveParams", "null"),
    m(
        "rail.rename",
        "agent_RenameParams",
        "agent_RailNode | agent_HeldAction",
    ),
    m("rail.startDoor", "agent_StartDoorParams", "agent_RailNode"),
    m("agent.run", "agent_RunParams", "terminal_Opened"),
    m("project.setWorktrees", "project_Worktrees", "null"),
    m("project.setMcp", "project_Mcp", "null"),
    m("project.get", "null", "project_ProjectSettings"),
    m("project.setModels", "project_ProjectModels", "null"),
    m("models.list", "null", "project_ModelList"),
    m("project.resources", "null", "project_Resources"),
    m("project.open", "project_OpenParams", "null"),
    m("project.rename", "project_RenameParams", "null"),
    m(
        "attachment.save",
        "attachment_SaveParams",
        "attachment_Saved",
    ),
    m("message.send", "message_SendParams", "message_Message"),
    m("message.pass", "message_MessageId", "message_Message"),
    m("message.get", "message_MessageId", "message_Message"),
    m("message.list", "message_ListParams", "message_Message[]"),
    m("message.deliver", "message_MessageId", "message_Message"),
    m("message.drop", "message_MessageId", "message_Message"),
    m("route.set", "message_SetRouteParams", "message_Route"),
    m("route.list", "null", "message_Route[]"),
    m("takeover.begin", "message_TakeoverParams", "null"),
    m("takeover.end", "message_TakeoverParams", "null"),
    m(
        "checkpoint.write",
        "checkpoint_WriteParams",
        "checkpoint_Checkpoint",
    ),
    m(
        "checkpoint.list",
        "checkpoint_ListParams",
        "checkpoint_Checkpoint[]",
    ),
    m("rail.remove", "agent_NodeId", "agent_Removal | null"),
    m(
        "agent.resume",
        "agent_NodeId",
        "agent_RailNode | agent_HeldAction",
    ),
    m("agent.setModel", "agent_SetModelParams", "agent_RailNode"),
    m(
        "workstream.clear",
        "workstream_ClearParams",
        "workstream_ClearResult",
    ),
    m(
        "workstream.rewindPlan",
        "workstream_RewindPlanParams",
        "workstream_RewindPlan",
    ),
    m(
        "workstream.rewind",
        "workstream_RewindParams",
        "workstream_Rewind",
    ),
    m("workstream.handoff", "workstream_HandoffParams", "null"),
];

/// Every method name the Daemon serves, sorted.
pub fn method_names() -> Vec<&'static str> {
    let mut names: Vec<_> = METHODS.iter().map(|m| m.name).collect();
    names.sort_unstable();
    names
}

/// FNV-1a 64 over `parts`, each followed by a zero byte, as 16 hex digits.
fn fnv1a<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for byte in part.bytes().chain([0]) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("{hash:016x}")
}

/// One method's hash (A38): FNV-1a 64 over its name, params and result. It is the method's own line in `contract.ts`.
pub fn method_hash(method: &Method) -> String {
    fnv1a([method.name, method.params, method.result])
}

/// The Contract fingerprint (A38): FNV-1a 64 over each method's name and `method_hash`, in name order.
/// The App derives it from the hashes `contract.ts` lists with the same function (`contractFingerprint`).
pub fn fingerprint_of_hashes(entries: &[(&str, &str)]) -> String {
    let mut sorted = entries.to_vec();
    sorted.sort_unstable_by_key(|(name, _)| *name);
    fnv1a(sorted.into_iter().flat_map(|(name, hash)| [name, hash]))
}

/// The fingerprint of `methods`, in any order.
pub fn fingerprint_of(methods: &[Method]) -> String {
    let hashes: Vec<String> = methods.iter().map(method_hash).collect();
    let entries: Vec<(&str, &str)> = methods
        .iter()
        .zip(&hashes)
        .map(|(method, hash)| (method.name, hash.as_str()))
        .collect();
    fingerprint_of_hashes(&entries)
}

/// The fingerprint of `METHODS`.
pub fn fingerprint() -> String {
    fingerprint_of(METHODS)
}

/// Render `methods` as `contracts/generated/contract.ts`: one line per method, in name order, holding that method's hash.
/// No line depends on any other method, so two changes to the contract merge without a conflict.
pub fn render_contract_typescript_of(methods: &[Method]) -> String {
    let mut sorted: Vec<&Method> = methods.iter().collect();
    sorted.sort_unstable_by_key(|m| m.name);
    let mut out = String::from("// Generated from crates/contracts/src/methods.rs. Do not edit.\n");
    out.push_str("export const CONTRACT = {\n  methods: {\n");
    for method in sorted {
        out.push_str(&format!(
            "    \"{}\": \"{}\",\n",
            method.name,
            method_hash(method)
        ));
    }
    out.push_str("  },\n} as const;\n");
    out
}

/// Render `METHODS` as `contracts/generated/contract.ts`.
pub fn render_contract_typescript() -> String {
    render_contract_typescript_of(METHODS)
}

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

    #[test]
    fn writes_contract_ts() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../contracts/generated/contract.ts"
        );
        std::fs::write(path, render_contract_typescript()).unwrap();
    }

    fn copy(methods: &[Method]) -> Vec<Method> {
        methods
            .iter()
            .map(|x| m(x.name, x.params, x.result))
            .collect()
    }

    #[test]
    fn a38_fingerprint_follows_the_methods_and_types() {
        let base = fingerprint();
        let mut reordered = copy(METHODS);
        reordered.reverse();
        assert_eq!(fingerprint_of(&reordered), base);
        let mut added = copy(METHODS);
        added.push(m("extra.method", "null", "null"));
        assert_ne!(fingerprint_of(&added), base);
        let mut retyped = copy(METHODS);
        retyped[0] = m(retyped[0].name, "other_Params", retyped[0].result);
        assert_ne!(fingerprint_of(&retyped), base);
    }

    #[test]
    fn a38_fingerprint_is_pinned_for_the_app_to_derive() {
        // The App's `contractFingerprint` pins the same literal (`a38_the_app_derives_the_fingerprint_rust_computes`).
        let entries = [("b.two", "0123456789abcdef"), ("a.one", "fedcba9876543210")];
        assert_eq!(fingerprint_of_hashes(&entries), "0595958c4791a090");
    }

    #[test]
    fn a38_a_change_to_one_method_rewrites_only_its_line() {
        let base = render_contract_typescript();
        let lines = |text: &str| -> std::collections::BTreeSet<String> {
            text.lines().map(str::to_owned).collect()
        };
        let mut added = copy(METHODS);
        added.push(m("extra.method", "null", "null"));
        let mut retyped = copy(METHODS);
        retyped[0] = m(retyped[0].name, "other_Params", retyped[0].result);
        for changed in [added, retyped] {
            let after = render_contract_typescript_of(&changed);
            assert_eq!(lines(&after).difference(&lines(&base)).count(), 1);
            assert!(lines(&base).difference(&lines(&after)).count() <= 1);
        }
    }

    fn method(name: &str) -> &'static Method {
        METHODS.iter().find(|m| m.name == name).unwrap()
    }

    fn generated(path: &str) -> String {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../contracts/generated/");
        std::fs::read_to_string(format!("{root}{path}")).unwrap()
    }

    #[test]
    fn g8_project_methods_are_registered() {
        let set = method("project.setWorktrees");
        assert_eq!((set.params, set.result), ("project_Worktrees", "null"));
        let get = method("project.get");
        assert_eq!(
            (get.params, get.result),
            ("null", "project_ProjectSettings")
        );
        assert!(generated("project/Worktrees.ts").contains("on: boolean"));
        assert!(generated("project/Worktrees.ts").contains("check: string | null"));
        assert!(generated("project/ProjectSettings.ts").contains("worktrees: Worktrees"));
    }

    #[test]
    fn g8_worktree_state_and_land_shapes() {
        let state = method("agent.worktreeState");
        assert_eq!(
            (state.params, state.result),
            ("agent_NodeId", "agent_WorktreeState")
        );
        let land = method("agent.land");
        assert_eq!((land.params, land.result), ("agent_NodeId", "agent_Landed"));
        assert!(
            generated("agent/WorktreeState.ts")
                .contains("{ ahead: number, behind: number, dirty: boolean, }")
        );
        assert!(generated("agent/Landed.ts").contains("{ base: string, }"));
    }

    #[test]
    fn d8_resources_method_and_types() {
        let resources = method("project.resources");
        assert_eq!(
            (resources.params, resources.result),
            ("null", "project_Resources")
        );
        assert!(generated("project/Resources.ts").contains("agents: Array<AgentResources>"));
        assert!(generated("project/Resources.ts").contains("total: ResourceTotal"));
        assert!(generated("project/AgentResources.ts").contains("disk_bytes: number | null"));
        assert!(generated("project/AgentResources.ts").contains("cpu_percent: number"));
    }

    #[test]
    fn g8_discard_is_registered() {
        let discard = method("agent.discard");
        assert_eq!((discard.params, discard.result), ("agent_NodeId", "null"));
    }

    #[test]
    fn g8_rail_node_worktree_is_required() {
        assert!(generated("agent/RailNode.ts").contains("worktree: Worktree | null,"));
        assert!(
            generated("agent/Worktree.ts")
                .contains("{ path: string, branch: string, base: string, }")
        );
    }

    #[test]
    fn g8_no_new_rpc_code() {
        let source = include_str!("../../rpc/src/error.rs");
        let names: Vec<&str> = source
            .lines()
            .filter_map(|l| l.trim().strip_prefix("pub const "))
            .filter_map(|l| l.split(':').next())
            .collect();
        assert_eq!(
            names,
            [
                "PARSE_ERROR",
                "METHOD_NOT_FOUND",
                "INVALID_PARAMS",
                "INTERNAL",
                "NOT_FOUND",
                "FORBIDDEN",
                "CONFLICT",
                "UNKNOWN_OUTCOME",
                "BUSY",
                "NOT_ACCEPTED",
                "NOT_RUNNING",
                "NOT_ACKED",
                "EVENTS_EXPIRED",
                "INBOX_UNAVAILABLE",
            ]
        );
    }
}
