# Loop rules

Module: `loop/rules.sh`, and `loop/percy.sh` for L36. Each id's cases are its `L<n>` tests in `loop/rules.test.sh`, `loop/merge-ready.test.sh` and `loop/percy.test.sh`.

| Exit | Means |
|---|---|
| 0 | holds |
| 1 | a rule is broken; stderr names it |
| 2 | bad input or usage, before any network call |
| 4 | `gh` failed or exceeded `BOXD_GH_TIMEOUT` (20 s); never read as a pass |

**L1 size.** `size` prints an advisory and still exits 0 when a PR spans more than one module directory or about 2000 changed lines, lockfiles and `generated/` excluded.

**L2 approval.** `trailers` passes when every commit carries `Author-Agent` and the newest is an empty `Reviewed-by-Agent` commit whose id differs from every author.

**L3 vocabulary.** `vocab` fails naming the word when a public Rust item, TS export or `contracts/` name holds a `GLOSSARY.md` _Avoid_ word, except under `crates/agents/claude_code/`.

**L33 base.** `base <pr>` passes only when the PR's base is `main`.

**L34 ready.** `ready` prints each `## Work` row of `scenarios/*.md` as the first that holds of `done` (every id has a test), `in-flight #<pr>` (an open PR's title names an id; gh missing, failing or slower than `BOXD_GH_TIMEOUT` exits 4, and `--offline` skips gh), `unspecified` (an id has no heading), `waiting` (an `After` id is not done) or `ready`.

**L36 Percy build.** On every PR touching `apps/desktop/src/`, and on each such push to `main` as Percy's baseline, the `visual` workflow's `percy` job serves `just harness` and runs `percy snapshot` on each seed of `.agents/data/harness.md` at 700 and 1280 px wide; it fails without `PERCY_TOKEN`, when the harness never answers, when Percy fails, or when its output has no build link, and writes that link to the job summary.

**L42 delta.** `delta <base-dir> <head-dir>` prints `<step> <id> fixed|regressed|still-failing|still-passing` per check and fails on any `regressed`; a check missing on one side is exit 2, never a pass.

**L44 class.** `class <pr>` prints `post` only when every changed file is docs prose or a scenario passing the content screen; anything else, or any doubt, is `block`.

**L45 rounds.** `rounds <pr>` fails at 3 independent rejects until an Architect picks `split` or `amend`; `retire` fails it for good.

**L46 merge-ready.** `merge-ready <pr>` prints `ready <head SHA>` only when `base`, `rounds`, the lane's trailer rule and `proof` hold and `check` and `rules` passed on that exact head.

**L53 proof.** `proof <pr>` passes only when the body has `## Shape` and `## Proof` in the format of `.agents/data/pr.md`; a visible PR adds L76's lines, never images.

**L54 carry.** `carry <pr>` keeps an approval across later merges of `main` that conflict nowhere and change nothing beyond the clean merge.

**L76 Percy proof.** A PR touching `apps/desktop/src/` passes `proof` only when the `percy` check (L36) succeeded on its exact head and Proof has a `Percy: https://percy.io/…/builds/<n>` line; one touching `crates/desktop/` needs a `macOS: <what was seen>` line, WKWebView being the one laptop check. No other path, scenario text or verdict asks for visual proof.

**L78 auto-merge.** The merge-ready workflow disarms auto-merge on a `.github/` PR only when it is enabled, and fails closed on a read or disarm error.

## Work

| Ids | Item | Owns | Keeps green | After |
|---|---|---|---|---|
| L41 | `tokens`: fail on a look literal outside `tokens.css` | `loop/rules.sh` | `L1`–`L3` | — |
| L43 | `ranges`: a squad range holds no used id | `loop/rules.sh` | — | — |
| L48 | `revert-due`: list post PRs whose newest verdict is a reject 60 minutes after merge | `loop/rules.sh` | `L46` | — |
| L49 | `dispatch <id>`: pass only when the scenario's newest change was approved | `loop/rules.sh` | `L34` | — |
| L50 | `queue`: two Work rows never name one id; every scenario file has `Module:` | `loop/rules.sh` | `L34` | — |
| L57 | `labels`: one `kind:`, `area:`, `lane:` matching `class`, `owner:` | `loop/rules.sh` | `L44` | — |
| L28 | `loop/stalls.sh check\|report`: detect, own and report each Stall once (red `main`, an unmerged approval, a third reject, a lost verdict, an overdue post-merge review) | `loop/stalls.sh` | — | — |
| L63 | a reject lists every finding; its count line gates `rounds` | `loop/rules.sh` | `L45` | — |
