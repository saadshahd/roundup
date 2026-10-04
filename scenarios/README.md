# Scenarios

A scenario is the spec for a unit of work: given / when / then, in `GLOSSARY.md` words. No scenario, no work.

- Ids are a letter and a number (`T3`). A test that proves a scenario has the lowercase id as the start of its name: `fn t3_completing_a_blocker_unblocks_the_todo`.
- A PR names the scenario ids it implements; rule 1 ("done") requires each to have a passing test whose name starts with its id. Two exceptions: W1 changes only root config, and U55 is a diagnosis that changes no app file. Each names the observer that proves it instead of a test.
- Methods and events are the ones in `crates/contracts`. Errors use the codes in `rpc::code`.
- Every write or read of a Todo or Pad is a Touch: `ctx.touch(verb, "todo:<id>" | "pad:<name>")`.
- Tests run without Claude, a network or a display. Anything that needs a real `claude` is replayed from `spikes/hooks-state/*.jsonl`.

Each scenario file states its module and ids on its `Module:` line, just under its title (`scenarios/loop.md` L50); this file holds no table of files, so adding a scenario file edits no line here.

The order of work, the PR each scenario belongs to and what can start now are in `.work/queue.md`.
