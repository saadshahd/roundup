# The one definition of "check": CI runs exactly this.
check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo nextest run
    cargo machete crates
    pnpm lint
    pnpm typecheck
    pnpm slop
