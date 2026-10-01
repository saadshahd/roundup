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
    just packages typecheck
    just packages test

# Fails when the generated TypeScript differs from the Rust contract (tests regenerate it).
contracts-fresh:
    git diff --exit-code -- contracts/generated
    test -z "$(git ls-files --others --exclude-standard contracts/generated)"

# Runs a script in every pnpm package under apps/ and ext/; packages without it are skipped, no packages is a pass.
packages script:
    pnpm -r --filter "./apps/*" --filter "./ext/*" --if-present {{script}}

# Builds rupd and rup, starts the webview dev server, runs the App on a folder.
app folder:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build -p rupd -p rup
    pnpm --filter "./apps/*" --if-present dev &
    dev=$!
    trap 'kill $dev' EXIT
    cargo run -p desktop -- {{folder}}
