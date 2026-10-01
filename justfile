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

# Builds rupd and rup, starts the webview dev server, waits until it answers, runs the App on a Project.
app project:
    #!/usr/bin/env bash
    set -euo pipefail
    path=$(cd {{quote(invocation_directory())}} && realpath -- {{quote(project)}})
    cargo build -p rupd -p rup
    set -m
    pnpm --filter "./apps/*" --if-present dev &
    dev=$!
    set +m
    trap 'kill -- -"$dev" 2>/dev/null || true' EXIT
    # The window loads devUrl once and does not retry, so it must not open before Vite answers.
    for _ in $(seq 150); do
        curl -sf -o /dev/null http://localhost:5173 && break
        kill -0 "$dev" 2>/dev/null || { echo "dev server exited before answering on :5173" >&2; exit 1; }
        sleep 0.2
    done
    curl -sf -o /dev/null http://localhost:5173 || { echo "dev server did not answer on :5173 within 30 s" >&2; exit 1; }
    cargo run -p desktop -- "$path"

# Builds the webview files, then rupd, rup and the App (custom-protocol, serving apps/desktop/dist) in release mode, and runs the App on them with no dev server. No bundling or signing.
app-release project:
    #!/usr/bin/env bash
    set -euo pipefail
    path=$(cd {{quote(invocation_directory())}} && realpath -- {{quote(project)}})
    pnpm -r --if-present build
    cargo build --release -p rupd -p rup
    cargo build --release -p desktop --features custom-protocol
    cargo run --release -p desktop --features custom-protocol -- "$path"
