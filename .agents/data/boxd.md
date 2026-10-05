# boxd

Data for the Driver and anyone starting a VM; nothing on the merge path needs it. Raw measurements: `spikes/boxd/REPORT.md`.

## Uses

| Use | Command | Verified |
|---|---|---|
| Unattended Builder | `BOXD_AGENT=claude loop/boxd.sh build <name> <prompt-file>` (`--isolated` VM), or `BOXD_AGENT=codex BOXD_CODEX_AUTH_VM=ru-<login-holder> …` (normal `ru-toolchain` VM, after guards prove no GitHub login) | Claude: a five-turn task. Codex: login and run; guards stub-tested |
| Reviewer | `loop/boxd.sh review <name> <prompt-file> <ref>` (same `BOXD_AGENT` split); the verdict lands in `loop/out/verdicts/<name>.md`, which the Driver posts with `gh pr comment <pr> --body-file …` before the approval commit | stub-tested |
| Linux check | `loop/boxd.sh check <pr\|branch>`: merges the ref into `origin/main` on the laptop, runs `just check` in an `--isolated` VM | ~2 min per PR; not the macOS gate |
| Parallel Builders | one VM each | up to 4 |
| QA screenshots | `agent-browser` in the VM, `boxd machine cp` out | static page only |
| Watch live | `boxd machine desktop <vm> --json` → `desktop_url` | URL issued, never viewed |

## Credentials

| VM | GitHub | Use only for |
|---|---|---|
| `--isolated` | none | Claude build, review, check; untrusted code |
| normal, from `ru-toolchain` | none (guards check `gh auth status` and `run github-app get-token` before upload) | Codex build and review |
| plain `boxd machine new` | the account's OAuth token via boxd's `gh` wrapper, plus App tokens (`boxd-app[bot]`, 50 repos) | Codex login holder, proof publishing, PR reads and comments; never a checkout |

The Claude token is the boxd secret `CLAUDE_CODE_OAUTH_TOKEN`: a VM holds a `bxds_` placeholder, swapped for the real token only on `*.anthropic.com`, `*.claude.com` and `claude.ai`. Never pass `-e CLAUDE_CODE_OAUTH_TOKEN=…`. Set `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` for `claude`, or it hangs ~90 s after answering. Should the repo ever run code outside its own PRs, `check` and `bake` VMs stop receiving the secret.

## Defaults

- Name `ru-…`; at most `BOXD_MAX_VMS` (default 12, measured to 4) at once.
- `--auto-destroy-timeout 4200` (`bake`: 7200) and `--auto-suspend-timeout 0`; `BOXD_AGENT_TIMEOUT` ≤ 1800 s via GNU `timeout`; auto-hibernate after 14400 s without traffic.
- From `ru-toolchain` (Rust, clippy, rustfmt, nextest, cargo-machete, just, pnpm, Node 24, WebKitGTK, warmed caches); rebake with `loop/boxd.sh bake` when `rust-toolchain.toml`, the pnpm pin or the lockfiles change a lot.
- Reboot a snapshot VM before use (`boxd machine reboot`, then wait for `exec -- true`): otherwise native tsc hangs on `fanotify` and `exec` blocks.
- Upload with `git archive`; bring results back with `git format-patch` and `boxd machine cp`; push from the laptop.
- Logs: `loop/out/runs/<name>-<UTC>-<pid>.*`, `loop/out/events.log`. A usage-limit reply writes `loop/out/PAUSED` and exits 75; delete it to resume.
- `loop/boxd.sh` destroys its own VMs; destroy hand-made QA VMs yourself.

## Cannot

Run WKWebView, give macOS perf numbers, exercise darwin PTYs or anything native, compare against macOS screenshots (fonts differ), or raise the shared usage quota: macOS behaviour comes from the laptop and CI.
