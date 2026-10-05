# Scenarios

A scenario is the spec for a unit of work: given / when / then, in `GLOSSARY.md` words.

- Ids are a letter and a number (`T3`); an id is taken when its heading exists. Each file states its module and ids on its `Module:` line, under its title (L50).
- A PR names the scenario ids it implements. Two prove by an observer instead of a test: W1 (root config only) and U55 (a diagnosis that changes no app file).
- Methods and events are the ones in `crates/contracts`; errors use the codes in `rpc::code`.
- Every write or read of a Todo or Pad is a Touch: `ctx.touch(verb, "todo:<id>" | "pad:<name>")`.
- Tests run without Claude, a network or a display; anything needing a real `claude` replays `spikes/hooks-state/*.jsonl`.
- Each file ends in a `## Work` table (`Ids | Item | Owns | Keeps green | After`); `loop/rules.sh ready` (L34) prints each row as `done`, `unspecified`, `waiting` or `ready`, and a done row is deleted. Who writes which table: `docs/squads.md`.

## Deferred past the MVP (no scenario yet)

- motion.md rows other than the Drawer slide and drag;
- Terminals named from their first command;
- the `● 1 below` line (U30 covers jumping, not the pinned line), provenance letters, the Todo `on` field and the Pad storage switch;
- Inbox, Messages, Routes, Extensions and history;
- packaging (a signed `.app`).

Product roadmap: [Rooms, Doors and Sketches](../docs/wireframes.md#accepted-product-direction-rooms-doors-and-sketches); it needs scoped scenarios before dispatch.
