# The one definition of "check": CI runs exactly this.
check: check-rust check-web

check-rust:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo nextest run
    just contracts-fresh
    cargo machete crates
    spikes/context-injection/e7_report.test.sh

check-web:
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

# Serves the real App on a fake Daemon holding the named seed (U26) and prints the URL (pass another port when 5199 is taken); no Daemon, Tauri or display needed.
harness seed="tree-40" port="5199":
    @echo "http://localhost:{{port}}/harness.html?seed={{seed}}"
    pnpm --filter desktop exec vite --port {{port}} --strictPort

# Opens the development App only after its imports transform; an HTML response alone can hide a Vite error.
app project:
    #!/usr/bin/env bash
    set -euo pipefail
    path=$(cd {{quote(invocation_directory())}} && realpath -- {{quote(project)}})
    # Anything already on :5173 would answer the wait below in place of our Vite and the window would open on the wrong webview.
    ! curl --max-time 1 -sf -o /dev/null http://localhost:5173 || { echo "something already answers on :5173; stop it first" >&2; exit 1; }
    CI=true pnpm install --frozen-lockfile
    cargo build -p rupd -p rup
    set -m
    pnpm --filter "./apps/*" --if-present dev &
    dev=$!
    set +m
    trap 'kill -- -"$dev" 2>/dev/null || true' EXIT
    # The window loads devUrl once and does not retry, so it must not open before Vite answers.
    deadline=$((SECONDS + 30))
    while (( SECONDS < deadline )); do
        kill -0 "$dev" 2>/dev/null || { echo "dev server exited before answering on :5173" >&2; exit 1; }
        curl --max-time 1 -sf -o /dev/null http://localhost:5173 && break
        sleep 0.2
    done
    (( SECONDS < deadline )) || { echo "dev server did not answer on :5173 within 30 s" >&2; exit 1; }
    curl --max-time 30 --fail-with-body -sS http://localhost:5173/__roundup_ready || { echo "webview imports are not ready; App was not opened" >&2; exit 1; }
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

# Rule 7 as a command: measures a fresh Daemon (median of 11 runs), writes target/perf.json, fails on a miss against crates/perf/budgets.json.
perf *args:
    cargo build --release -p rupd -p rup -p perf
    ./target/release/perf {{args}}

# Keystroke-to-render p95 in the real App's WKWebView (macOS only; scenarios/perf.md K1-K3, R11). Builds the App with the probe into target/perf-app and opens its window, kept above every other window and Space, for about 40 s per run, and refuses unless ROUNDUP_ALLOW_WINDOW=1.
perf-keystroke *args:
    #!/usr/bin/env bash
    set -euo pipefail
    [ "${ROUNDUP_ALLOW_WINDOW:-}" = 1 ] || { echo "perf-keystroke opens a window over your screen for about 40 s per run and types into it; set ROUNDUP_ALLOW_WINDOW=1 only when the user asked for the run" >&2; exit 2; }
    dist={{quote(justfile_directory())}}/target/perf-dist
    VITE_ROUNDUP_PERF=1 pnpm --filter desktop exec vite build --outDir "$dist" --emptyOutDir
    TAURI_CONFIG=$(jq -nc --arg dist "$dist" '{build: {frontendDist: $dist}, app: {windows: [{title: "roundup", alwaysOnTop: true, visibleOnAllWorkspaces: true}]}}') \
        cargo build --release -p rupd -p desktop --features desktop/custom-protocol --target-dir target/perf-app
    cargo build --release -p perf
    ./target/release/perf-keystroke {{args}}
