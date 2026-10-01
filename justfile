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

# Runs a script in every pnpm workspace package (apps/*, ext/*); packages without it are skipped, no packages is a pass.
packages script:
    pnpm -r --if-present {{script}}

# Builds rupd and rup, starts the webview dev server, runs the App on a folder.
app folder:
    #!/usr/bin/env bash
    set -euo pipefail
    project=$(cd {{quote(invocation_directory())}} && realpath -- {{quote(folder)}})
    cargo build -p rupd -p rup
    set -m
    pnpm --filter "./apps/*" --if-present dev &
    dev=$!
    set +m
    trap 'kill -- -"$dev" 2>/dev/null || true' EXIT
    cargo run -p desktop -- "$project"
