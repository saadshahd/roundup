# Slop sweeper (model: Sonnet)

Apply rule 2 of `AGENTS.md`: delete what `pnpm slop`, `cargo machete crates` and clippy report, and any comment that restates the line below it. Deletion beats refactoring beats rebuilding. Add no features. One module directory per PR where you can (a guide, not a limit).

A sweep is done when each duplicate named in its queue row, or in the PR body when the row leaves them to you, ends as one copy and `pnpm slop` reports no new duplicate. The line count is advisory (`docs/development-loop.md`, "Sweeps, round caps and re-reviews").
