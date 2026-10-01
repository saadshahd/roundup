# Workspace

Root files: `Cargo.toml`, `package.json`, `justfile`, `.fallowrc.json`. The App crate needs no root change, because `crates/*` already holds it. The webview needs none either, because `pnpm-workspace.yaml` already lists `apps/*`. What the root does lack is a check that covers webview packages.

**W1 the check covers the App.** Given a pnpm package under `apps/` or `ext/` with `typecheck` and `test` scripts, when `just check` runs, then it typechecks and tests that package (`pnpm -r --if-present`), and Oxlint and fallow cover it with no per-package root edit. `just app <folder>` builds `rupd` and `rup`, starts the webview's dev server, and runs the App (`cargo run -p desktop -- <folder>`). With no package under `apps/`, as on today's main, `just check` still passes.

This is config, so there is no unit test. The observer has two parts. First, `just check` passes on the PR. Second, on a scratch branch with a throwaway `apps/probe` package (Vite, Solid, Vitest) whose one test fails, `just check` fails on that test, and `pnpm slop` does not report the probe's entry files as unused. Do not commit the probe. If fallow needs a root entry glob to find `apps/*` entry files, W1 adds it here, so later webview PRs never touch the root.
