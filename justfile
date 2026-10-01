# The one definition of "check": CI runs exactly this.
check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo nextest run
    just contracts-fresh
    cargo machete crates
    pnpm lint
    pnpm typecheck
    pnpm slop
    node scripts/vocab.mjs

# Fails when the generated TypeScript differs from the Rust contract (tests regenerate it).
contracts-fresh:
    git diff --exit-code -- contracts/generated
    test -z "$(git ls-files --others --exclude-standard contracts/generated)"

# AGENTS.md rule 3 for the current branch.
pr-size base="origin/main":
    node scripts/pr-size.mjs {{base}}
