# Work queue: the shortest path to the MVP

This is the queue until roundup can hold its own Todos (`docs/development-loop.md`). Each line holds one scenario: `id · module · PR · merge after · start · owner`. Scenarios with the same PR key ship as one PR, built by one Builder in its own worktree. That Builder edits only the files its PR owns (table below). Line counts are estimates per PR, excluding lockfiles and generated files, sized to the guide of about 2000 lines (#39).

- **now**: can start today.
- **stacked**: start now on top of the named branch, and merge after it.
- **after X**: start once X is merged.

## PRs

| PR | Module | Scenarios | Owns | Est. lines | Merge after | Start |
|---|---|---|---|---|---|---|
| `contract-spawn-terminal` (#43) | `crates/contracts` | serves A10, U9 | `agent.rs`, `methods.rs` | ~40 | none | now (architect-1 authors; a different id approves, rule 4) |
| `contract-schemas` (#44) | `crates/contracts` | serves M1 | `Cargo.toml`, `todo.rs`, `pad.rs`, `common.rs` | ~40 | none | now (same) |
| `workspace` | root | W1 | `package.json`, `justfile`, `.fallowrc.json` | ~40 | none | now |
| `daemon-attached` | `crates/rupd` | D1 | `crates/rupd/**` | ~200 | none | now |
| `app` | `crates/desktop` | S1–S3 | `crates/desktop/**` (new crate; `crates/*` already globs it) | 1000–1400 | none | now |
| `ui-shell` | `apps/desktop` | U1–U5 | package and build files, `src/main.tsx`, `src/App.tsx`, `src/app/**` (App seam adapter, typed calls), `src/state/**`, `src/ink/**`, `src/drawer/**`, and one placeholder per region: `src/rail/Rail.tsx`, `src/terminal/Pane.tsx`, `src/todos/Todos.tsx`, `src/pads/Pads.tsx` | 1500–1900 | `workspace` | now |
| `ui-rail` | `apps/desktop` | U6–U10 | `src/rail/**` | 1400–1800 | `ui-shell`, `contract-spawn-terminal` | after `ui-shell` |
| `ui-terminal` | `apps/desktop` | U11–U14 | `src/terminal/**`; also `package.json` and `pnpm-lock.yaml` to add the xterm packages. It is the only parallel UI PR that touches those two files | 1000–1400 | `ui-shell` | after `ui-shell` |
| `ui-todos` | `apps/desktop` | U15–U17 | `src/todos/**` | 1200–1600 | `ui-shell` | after `ui-shell` |
| `ui-pads` | `apps/desktop` | U18–U21 | `src/pads/**` | 1000–1400 | `ui-shell` | after `ui-shell` |
| `agents-mvp` | `crates/agents` | A9–A12 | `crates/agents/**` | 800–1100 | agents stack #17–#37, `contract-spawn-terminal` | stacked on `builder/agents-11` |
| `mcp` | `crates/rup` | M1–M3 | `crates/rup/src/**`, `crates/rup/Cargo.toml`, `crates/rup/tests/mcp*` | 700–1000 | `contract-schemas`, #34 (it rewrites `crates/rup/src/main.rs`) | after `contract-schemas`; branch from `builder/agents-11` until #34 merges |
| `daemon-e2e` | `crates/rup` | D2–D4 | `crates/rup/tests/e2e*` | 600–900 | `agents-mvp`, `mcp` | after both |
| `ui-drag` (stretch) | `apps/desktop` | U22 | `src/rail/drag*` | 600–900 | `ui-rail` | after `ui-rail`, only if drag is in the MVP |

Shared webview files (`src/app`, `src/state`, `src/ink`, `src/drawer`, `src/App.tsx`, `package.json`) belong to `ui-shell`. A feature PR that needs to change one stops and reports it; the change becomes a `ui-shell` follow-up, never an edit inside the feature PR. That rule is what lets four UI Builders run at once.

## Scenarios

- [ ] contract `rail.spawnTerminal` · `crates/contracts` · `contract-spawn-terminal` · none · now · architect-1
- [ ] contract JSON schemas for Todo and Pad params · `crates/contracts` · `contract-schemas` · none · now · architect-1
- [ ] W1 the check covers the App · root · `workspace` · none · now · —
- [ ] D1 attached · `crates/rupd` · `daemon-attached` · none · now · —
- [ ] S1 open a Project · `crates/desktop` · `app` · none · now · —
- [ ] S2 calls and events · `crates/desktop` · `app` · none · now · —
- [ ] S3 lifetime · `crates/desktop` · `app` · none (D1 matters only at run time; tests fake `rupd`) · now · —
- [ ] U1 calls and events · `apps/desktop` · `ui-shell` · `workspace` · now · —
- [ ] U2 first run · `apps/desktop` · `ui-shell` · `workspace` · now · —
- [ ] U3 Rail state · `apps/desktop` · `ui-shell` · `workspace` · now · —
- [ ] U4 ink · `apps/desktop` · `ui-shell` · `workspace` · now · —
- [ ] U5 Drawer and last touch · `apps/desktop` · `ui-shell` · `workspace` · now · —
- [ ] U6 rows · `apps/desktop` · `ui-rail` · `ui-shell` · after `ui-shell` · —
- [ ] U7 live line · `apps/desktop` · `ui-rail` · `ui-shell` · after `ui-shell` · —
- [ ] U8 folding · `apps/desktop` · `ui-rail` · `ui-shell` · after `ui-shell` · —
- [ ] U9 actions · `apps/desktop` · `ui-rail` · `ui-shell`, `contract-spawn-terminal` · after `ui-shell` · —
- [ ] U10 Dock badge · `apps/desktop` · `ui-rail` · `ui-shell` · after `ui-shell` · —
- [ ] U11 one emulator per Terminal · `apps/desktop` · `ui-terminal` · `ui-shell` · after `ui-shell` · —
- [ ] U12 typing · `apps/desktop` · `ui-terminal` · `ui-shell` · after `ui-shell` · —
- [ ] U13 size · `apps/desktop` · `ui-terminal` · `ui-shell` · after `ui-shell` · —
- [ ] U14 pane header · `apps/desktop` · `ui-terminal` · `ui-shell` · after `ui-shell` · —
- [ ] U15 Todo list · `apps/desktop` · `ui-todos` · `ui-shell` · after `ui-shell` · —
- [ ] U16 create a Todo · `apps/desktop` · `ui-todos` · `ui-shell` · after `ui-shell` · —
- [ ] U17 Todo detail · `apps/desktop` · `ui-todos` · `ui-shell` · after `ui-shell` · —
- [ ] U18 Pad list and ownership · `apps/desktop` · `ui-pads` · `ui-shell` · after `ui-shell` · —
- [ ] U19 create a Pad · `apps/desktop` · `ui-pads` · `ui-shell` · after `ui-shell` · —
- [ ] U20 open and edit a Pad · `apps/desktop` · `ui-pads` · `ui-shell` · after `ui-shell` · —
- [ ] U21 export a Pad · `apps/desktop` · `ui-pads` · `ui-shell` · after `ui-shell` · —
- [ ] A9 name from the first prompt · `crates/agents` · `agents-mvp` · #17–#37 · stacked on `builder/agents-11` · —
- [ ] A10 a Terminal in the Rail · `crates/agents` · `agents-mvp` · #17–#37, `contract-spawn-terminal` · stacked on `builder/agents-11` · —
- [ ] A11 Todos and Pads over MCP · `crates/agents` · `agents-mvp` · #17–#37 · stacked on `builder/agents-11` · —
- [ ] A12 a reopened Rail never names another run's Terminal · `crates/agents` · `agents-mvp` · #17–#37 · stacked on `builder/agents-11` · —
- [ ] M1 tools · `crates/rup` · `mcp` · `contract-schemas`, #34 · after `contract-schemas` · —
- [ ] M2 a call is a Touch by that Agent · `crates/rup` · `mcp` · `contract-schemas`, #34 · after `contract-schemas` · —
- [ ] M3 no Daemon, no hang · `crates/rup` · `mcp` · `contract-schemas`, #34 · after `contract-schemas` · —
- [ ] D2 Status end to end · `crates/rup` · `daemon-e2e` · `agents-mvp` · after `agents-mvp` and `mcp` · —
- [ ] D3 Todos over MCP end to end · `crates/rup` · `daemon-e2e` · `agents-mvp`, `mcp` · after `agents-mvp` and `mcp` · —
- [ ] D4 ten idle Agents · `crates/rup` · `daemon-e2e` · `agents-mvp` · after `agents-mvp` and `mcp` · —
- [ ] U22 drag (stretch) · `apps/desktop` · `ui-drag` · `ui-rail` · after `ui-rail` · —

## In flight elsewhere (reviewed by others; listed for the dependencies above)

- #17–#37: the agents stack (A1–A8). `agents-mvp` stacks on its top, `builder/agents-11`. Open defect (reported on #28): the stack persists `terminal_id`, but Terminal ids restart with every Daemon run, so a reopened Rail can point at a new, unrelated Terminal. A12 specifies the fix if the stack does not take it.
- #36 pads-4 (P5, P8, P9). #40 terminal-6 (X9, X10). #39 makes PR size a guide.

## Parallelism

- **Start now:** `contract-spawn-terminal` and `contract-schemas` (Architect), plus five Builders: `workspace`, `daemon-attached`, `app`, `ui-shell` (rebase after `workspace` merges) and `agents-mvp` (stacked). `mcp` joins as soon as `contract-schemas` merges.
- **Peak:** once `ui-shell` merges, `ui-rail`, `ui-terminal`, `ui-todos` and `ui-pads` run at once, alongside `mcp` and `agents-mvp`: six Builders.
- **Critical path:** `workspace` → `ui-shell` → the four UI PRs → MVP gate. In parallel: agents stack → `agents-mvp` → `daemon-e2e`.

## Branching notes

- `agents-mvp` stacks on `builder/agents-11`, which predates #43. Merge `origin/main` into the branch once #43 is on main; A10 needs `SpawnTerminalParams`. The stack already merges main into its branches this way.
- `mcp` needs the `JsonSchema` derives from #44. Branch from main once #44 is merged. If #34 is still open then, branch from `builder/agents-11` and merge `origin/main` into it.
- `app` must pass `just check` on a fresh clone with no webview build. Do not commit a `dist/`, and do not add a root build step to satisfy Tauri's `frontendDist` or its icons.

## Linux VMs and CI after `app` merges

Once `crates/desktop` is a workspace member, every `just check` compiles Tauri. That has two effects.

- **macOS CI gets slower.**
- **boxd checks break.** On Linux, Tauri needs WebKitGTK and its dev packages. The `ru-toolchain` snapshot has none, so every boxd `just check` fails from then on, in every module. The `app` Builder cannot run on a VM at all.

Default until the user decides (open question): run `app` and every later PR in local worktrees. Alternatively, rebake the snapshot with Tauri's Linux packages (`loop/boxd.sh bake`) before `app` merges.

## MVP gate (QA and Driver; no Builder)

Once every PR above except `ui-drag` is merged:

1. QA runs `just app <folder>` on macOS with real `claude`, drives U2–U21, and saves `artifacts/ux/<scenario>/<step>.png`. This is the Phase 3 screenshot pack.
2. QA measures on the laptop:
   - cold start;
   - keystroke-to-render p95 under bursty Agent output (the ADR 0001 caveat);
   - RSS with 10 idle real Agents.
3. The Driver writes the gate report (`docs/development-loop.md`, "Human gates").
