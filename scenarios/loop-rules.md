# Loop rules

Module: `loop/rules.sh`. Each id's cases are its `L<n>` tests in `loop/rules.test.sh` and `loop/merge-ready.test.sh`.

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

**L42 delta.** `delta <base-dir> <head-dir>` prints `<step> <id> fixed|regressed|still-failing|still-passing` per check and fails on any `regressed`; a check missing on one side is exit 2, never a pass.

**L44 class.** `class <pr>` prints `post` only when every changed file is docs prose or a scenario passing the content screen; anything else, or any doubt, is `block`.

**L45 rounds.** `rounds <pr>` fails at 3 independent rejects until an Architect picks `split` or `amend`; `retire` fails it for good.

**L46 merge-ready.** `merge-ready <pr>` prints `ready <head SHA>` only when `base`, `rounds`, the lane's trailer rule and `proof` hold and `check` and `rules` passed on that exact head.

**L53 proof.** `proof <pr>` passes only when the body has `## Shape` and `## Proof` in the format of `.agents/data/pr.md`, with images for a visible PR.

**L54 carry.** `carry <pr>` keeps an approval across later merges of `main` that conflict nowhere and change nothing beyond the clean merge.

**L76 unchanged rendering.** A visible-path PR needs no proof branch when the newest independent approve on the same tree carries `Visual: unchanged`.

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
