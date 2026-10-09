---
name: percy-review
description: "Poll the latest Percy build for the current branch until it completes, fetch Percy's AI region data + cluster summary + per-comparison bug flags, and reconcile each comparison against the captured Percy Visual Intent plus the developer's code context. Produces a verdict per comparison — expected, ambiguous, likely-bug, or unplanned-change — using agent semantic reasoning over Percy's AI text outputs, not keyword matching."
argument-hint: "[optional: build ID; defaults to latest build on current branch]"
---

# Percy Review

The core review loop. Polls a Percy build, gathers all Percy's AI signal (per-region change titles + descriptions + `change_reason` + `visual_quality`, cluster narratives, per-comparison bug flags), enriches it with the developer's code context (git diff, PR body, commit messages, conversation), and asks the agent to reason — **per comparison** — whether each diff matches stated intent.

This skill **does not** approve or reject builds. It produces verdicts. `/percy:gate` consumes them.

## Design principle — Percy-AI-native

The verdict logic runs entirely on **Percy's AI text outputs reconciled against code context**. Two reasons:

1. **Percy's AI already encodes the signal in text.** Each flagged region carries `change_reason` — Percy's own natural-language judgment, e.g. *"This is a deliberate CSS color change… not an expected dynamic variation"* (deliberate) vs *"could represent… an unintended CSS regression"* (unintended). Percy's classifier has already examined the rendered output and written down what it found, so reasoning over that text plus code context works directly from Percy's own conclusions.
2. **Percy-native signal.** Percy's per-region `change_reason` / `change_description` / `visual_quality_reason` come from Percy's own visual classifier — a richer, more specific signal than anything a downstream generic pass could reconstruct. The plugin is designed around that signal.

This skill therefore:
1. Gathers all raw Percy AI data (Steps 4–5).
2. Applies the mechanical signals: cluster-level consistency promotion (Step 6) + flaky-test pre-check (Step 6b).
3. Enriches with the developer's code context (Step 7b).
4. Hands the structured data to the agent and instructs it to reason per-comparison over Percy's text outputs + intent + code context (Step 8).

> **Percy's text is the only evidence — by design.** Verdicts come from Percy's AI text + the developer's code context, and nothing else. Do not add any further inference pass on top: a case that can't be resolved from text + code + intent becomes `needs-human`, not a guess.

## Prerequisites

- `.percy/config.yml` exists (run `/percy:setup`).
- A Percy Visual Intent section exists in the current PR description, OR `.percy/intent-<branch>.md` exists, OR there is enough conversation context to reconstruct intent.

## Endpoint cheat-sheet

```
GET /api/v1/builds?filter[branch]=<branch>&page[limit]=1     # find latest build
GET /api/v1/builds/<id>?include=build-summary                # build state + cluster summary
GET /api/v1/comparisons/<id>?include=browser.browser-family  # per-comparison regions + bugs
GET /api/v1/snapshots/<id>                                   # snapshot display-name
GET /api/v1/snapshots?build_id=<id>&page[limit]=100          # all snapshots for build
GET /api/v1/snapshots?project_id=<id>&filter[name]=<n>&page[limit]=20  # snapshot history for flaky detection (v0.2)
GET /api/v1/network-logs?comparison_id=<id>                  # smart-debug / asset-capture (v0.2; needs user token)
```

Most calls work with `Authorization: Token token="$PERCY_TOKEN"` and `User-Agent: curl/8.7.1` (Cloudflare blocks default UAs).
**Note:** `/network-logs` requires user-token auth (BrowserStack `PERCY_USERNAME` + `PERCY_ACCESS_KEY` via HTTP Basic), not the project `PERCY_TOKEN`. Same auth class used by `/reviews`.

## Steps

### 1. Resolve the target build

- If `$ARGUMENTS` is a build ID, use it.
- **If `.percy/config.yml` says `repo_role: regression-suite`**, resolve the latest build from the configured **`regression_branch`** — NOT the current checkout's branch (a QA engineer's local checkout is often `main` or a feature branch while the scheduled runs build `pre_prod`).
- Otherwise: `GET /builds?filter[branch]=<current branch>&page[limit]=1` and use the first result.
- **Ambiguous non-PR build:** if the resolved build is a non-PR build on a non-default branch and no `regression_branch` is configured, ask — "is this your scheduled regression run?" — instead of falling into NO-INTENT mode. On yes, treat it as regression mode (Step 7.4) and suggest persisting the branch via `/percy:setup`.
- If no build exists, stop and tell the user to push or run `/percy:run-build`.

### 2. Poll until done

Poll `GET /builds/<id>` every 15 seconds. Acceptable terminal states:
- `attributes.state ∈ {finished, finished_processing, failed}`
- AND `attributes.ai-details.all-ai-jobs-completed === true`

Stop on `failed` and report `failure-reason`. Timeout after 15 minutes — surface to the user and ask whether to keep waiting.

**One more gate before Step 4:** `all-ai-jobs-completed: true` does NOT mean the cluster summary is ready — `ai-details.summary-status` can still read `"processing"` for a while after. Keep polling until `summary-status ∈ {ok, skipped}` before fetching the build summary, or Step 4 reads an empty include.

Print one status line per poll (e.g., `polling build #41 · state=processing · ai-jobs=in-progress · 45s elapsed`). Don't spam.

### 3. AI coverage probe

Read from `attributes.ai-details`:

| Signal | Value | What it means |
|---|---|---|
| `total-comparisons-with-ai > 0` | ✅ | AI processed this build — proceed |
| `total-comparisons-with-ai === 0` AND `summary-status === "skipped"` AND `summary-reason === "no_ai_changes"` | OK | Build had no AI changes (e.g., identical to baseline) — likely PASS |
| `total-comparisons-with-ai === 0` AND `summary-reason` is something else | ⚠️ | No AI coverage — first check **Cold start** (no approved baseline yet); if neither cold-start case matches, fall back to NO-AI mode |

**Don't gate on `ai-details.ai-enabled`** — that's the auto-approve toggle, not "is AI available."

### 4. Pull cluster summary

```
GET /api/v1/builds/<id>?include=build-summary
```

- `summary-status === "ok"`: find `included[]` entry with `type === "build-summaries"`. Read `attributes.summary` — it is **either a JSON-encoded string or an already-parsed array** (handle both shapes; parse only if it's a string) → array of clusters:
  ```json
  [{
    "title": "Top-center 'todos' header changed from bright blue to muted red/maroon across screen sizes.",
    "occurrences": 3,
    "snapshots": [
      { "snapshot_id": "...", "comparisons": [{ "comparison_id": N, "width": W, "height": H, "bounding_boxes": [...] }] }
    ]
  }]
  ```
- `summary-status === "skipped"` AND `summary-reason === "too_many_comparisons"`: no cluster summary. Skip Step 5's cluster promotion — treat every comparison independently.
- `summary-status === "skipped"` AND `summary-reason === "no_ai_changes"`: build has no AI-detected changes; verdict = PASS.

### 5. Fetch per-comparison region detail

For every unique `comparison_id` referenced in clusters:

```
GET /api/v1/comparisons/<id>?include=browser.browser-family
```

From the comparison response, collect:
- `attributes.width`, `attributes.diff-ratio`
- `attributes.ai-details.total-potential-bugs`
- `attributes.applied-regions[]` — for each region, keep:
  - `change_title` (e.g., "Heading color changed")
  - `change_description` (e.g., "The large 'todos' heading at the top-center changed color from bright blue to a dark red/brown...")
  - `change_reason` (e.g., "This is a deliberate CSS color change...")
  - `change_type` (e.g., "CSS Style Changes")
  - `visual_quality` — **`"valid"` = not a bug**, `"irregularity"` = bug-flagged
  - `visual_quality_reason` (e.g., "Red borders/shadow suggest an error or unintended focus style...")
  - `priority`
  - `coordinates`
  - `ignored` (skip the region if `true`)
- Browser family name from `included[].browser-families[].attributes.name`
- Snapshot display-name — **don't fetch snapshots one at a time and don't hunt for a comparison→snapshot relationship key.** Before this step, prefetch the whole build's snapshots in ONE call — `GET /api/v1/snapshots?build_id=<id>&page[limit]=100` — and build a `snapshot_id → name` map. The cluster summary already carries each comparison's `snapshot_id`, so every name lookup after that is a local map hit. (Why the mapping exists at all: Percy's cluster/comparison payloads reference snapshots by id only; names live on the snapshot object — and names are what humans + deep-links need.) **Paginate past `page[limit]=100`** when the build has more snapshots — nightly regression suites routinely exceed 100; follow `links.next` until exhausted, or the map silently drops names.

### 6. Apply the ONE mechanical rule: cluster-level bug promotion

For each cluster K:
- Let `bug_flagged_K = ANY comparison in K has at least one applied-region with visual_quality === "irregularity" AND ignored !== true`

Rationale: Percy's AI is observably inconsistent across comparisons (same defect flagged in one comparison, not another). If any comparison in a cluster shows the defect as `irregularity`, treat the whole cluster as bug-flagged. The cluster groups comparisons that all show the same change — so the defect is logically present in all of them.

**Edge case:** if `summary-status === "skipped"` (no clusters available), skip promotion. Treat each comparison's regions individually.

**Edge case:** a single comparison can belong to multiple clusters when it visually contains multiple distinct changes. Promotion applies per-cluster independently — a comparison inherits "bug" status from each of its clusters separately, not collectively. **Scope the promotion by region ids, not whole comparisons:** each cluster entry carries `bounding_boxes` (short-code ids like `"EFCT40"`) per comparison, and these match `applied-regions[].id` on the comparison object. `bug_flagged_K` should therefore test only the regions whose `id` is in cluster K's `bounding_boxes` for that comparison. Without this, a comparison sitting in two clusters (e.g. an intent-matched recolor cluster AND an anomaly cluster) bleeds the anomaly's `irregularity` flag into the innocent cluster and bug-promotes every comparison in it.

### 6b. Flaky-test & dynamic-content pre-check (v0.2)

Before doing intent reconciliation, screen each `irregularity`-flagged comparison against snapshot **history** for two non-bug patterns: **flaky test captures** (snapshots taken before the page finished loading — blank screens, spinners, missing-section renders) and **dynamic content** (a surface that legitimately changes every build — carousels, feeds, timestamps, rotating assets). In practice, most misses of this kind are flaky captures, and some flagged surfaces are dynamic content (see the worked example below) that reads as a bug only because history wasn't in the loop.

For each comparison the cluster-promotion or per-region rule flagged as a candidate bug, run this check:

1. **Query snapshot history (project token only — no DB access).** There is no single-call "history by name" endpoint — `/snapshots` requires a `build_id` (a project-scoped `filter[name]` call returns `400 param is missing: build_id`). So assemble the history client-side, entirely with the project `PERCY_TOKEN`:
   1. List the project's recent builds on this branch: `GET /api/v1/builds?filter[branch]=<branch>&page[limit]=20`.
   2. For each build, fetch its snapshots: `GET /api/v1/snapshots?build_id=<build_id>&page[limit]=100`, keeping those whose `name` / `display-name` matches the comparison's snapshot.
   3. From each matched snapshot, read `diff-ratio`, `review-state`, `review-state-reason`, and `fingerprint` to build the per-build history.

   > **No database dependency.** The plugin runs entirely on the public REST API.

   > **Fetch once, reuse everywhere.** The recent-builds list and each build's snapshot pages are the same data for every flagged comparison — fetch this history **ONCE per run**, cache it, and reuse it across all comparisons this step screens. Never re-fetch per-comparison (a 20-build × N-flag run would multiply identical calls). And as in Step 5: paginate past `page[limit]=100` per build — nightly suites routinely exceed 100 snapshots, and a truncated history silently breaks the pattern thresholds.

2. **Detect flaky patterns** in the history:
   - **Pattern A — Recurring incomplete-render:** how many of the last N captures had high `diff-ratio` (>0.05) followed by an approval? If ≥ 30% → flaky-test signal.
   - **Pattern B — Fingerprint instability:** how many distinct snapshot fingerprints in the last N builds? If > 50% are unique (vs. clustering around 1-2 stable fingerprints) → flaky-test signal.
   - **Pattern C — Same diff seen before:** does the head fingerprint of this comparison match any prior build's *unreviewed* or *rejected* snapshots that were later resolved by re-running? If yes → flaky-test signal.
   - **Pattern D — Recurring approved change (dynamic content):** did this snapshot show a diff that was then **approved** in a clear majority (≥ ~60%) of the last N builds? A surface that changes and gets accepted nearly every build is **dynamic content** (carousel/feed/timestamp/rotating asset), not a regression — even when the current capture's `change_reason` reads "broken" (a half-loaded carousel frame looks like a defect but recurs every build). Threshold is tunable.

3. **If a pattern hits**, downgrade and **always surface for the user — never auto-approve and never auto-reject**:
   - **Flaky (Patterns A–C):** verdict `likely-flaky-test` — *"This snapshot was [blank / incomplete / spinner-rendered] in [8 of last 20] builds. Likely a wait-condition issue in the test, not a bug. Recommend fixing the test's page-load wait."*
   - **Dynamic content (Pattern D):** verdict `likely-flaky-test` with a dynamic-content reason — *"This surface changed and was approved in [N of last 20] builds — looks like dynamic content (e.g. a rotating carousel/feed), not a regression. Likely safe to approve/ignore — confirm?"*

   In both cases: surface the dashboard deep-link, attach the recommendation, and **ask the user to confirm before anything is approved** (this tier is excluded from `/percy:gate` auto-approve). If this is the *only* change in the build, still surface it for the user to approve — do not silently pass it. Skip Step 8's intent reconciliation for this comparison.

   > **Worked example (illustrative).** A PR's intent is inert reliability work ("add font load retry"); Percy flags bottom-carousel thumbnails as spinner/placeholder "irregularities," and the change didn't match the inert reliability intent — so without history the plugin flags it as a likely-bug (correct, given only intent + code). But the snapshot history shows those carousel thumbnails change-and-get-approved in most recent builds → **dynamic content** (Pattern D). With history in the loop the right output is `likely-flaky-test` (dynamic) surfaced for a one-click approve — not a bug. This class of case cannot be resolved without history — which is why this pre-check matters.

4. **If no pattern hits**, continue to Step 7 (Load Visual Intent).

### 7. Load Visual Intent

Priority:
1. **PR description** — `gh pr view --json body`, extract content between `<!-- percy-visual-intent: ... -->` markers.
2. **Local intent file** — `.percy/intent-<branch>.md`.
3. **Conversation context** — reconstruct from current session (only if it covers the changes in the build).
4. **Regression mode — accumulated intent from merged PRs.** A build is a **regression run** when it's on the configured **`regression_branch`** from `.percy/config.yml` (set by `/percy:setup` — often a dedicated branch like `pre_prod` or `qa`, NOT the default branch), or it's non-PR-linked on the default branch, or the user says so. The expectation is NOT simply "nothing changed": PRs merged since the last approved baseline *legitimately* changed the UI, and the test suite may have been updated to match. Build the intent like this:
   1. **Find the last approved build on the regression branch** (walk `GET /builds?filter[branch]=<regression_branch>` for the most recent `review-state: approved`) — keep both its commit SHA and its **`finished-at` timestamp**.
   2. **List the PRs merged in the gap.**
      - **Same repo** (app + suite live together): a SHA range works — `git log <last>..<head> --merges` / `gh pr list --state merged --base <regression_branch>`.
      - **Cross-repo — `app_repos` configured in `.percy/config.yml`: harvest with a TIME WINDOW, not a SHA range.** QA-repo SHAs don't exist in the app repos, so a SHA range is meaningless there. Window = last approved regression build's `finished-at` → this build's `created-at`. Per app repo: `gh pr list -R <owner/name> --state merged --search "merged:<window-start>..<window-end>"`.
   3. **Harvest each one's `## Percy Visual Intent` — following the promotion hop if needed.** On many teams the regression branch receives **bot-created merge PRs** (e.g. "Merge feat/xyz — by automation") whose bodies carry no intent; the intent lives on the **original feature PR**. Resolve it: merge PR → source branch name → `gh pr list --head <branch> --state merged` → that PR's `## Percy Visual Intent` section. The **union** of the harvested sections is this run's intent: intended = everything the merged PRs declared (tag each clause with its source PR#); invariant = everything else.
   4. **Zero intent sections found? Degrade LOUDLY — don't declare everything suspect.** Say up front: *"No Percy Visual Intent sections found in `<app_repos>` — either nothing merged or the app team doesn't use this plugin. Falling back to PR titles/bodies as weak intent (lower confidence)."* Then use the merged PRs' **titles + bodies as a weak-intent tier**: a diff that plausibly matches one is `expected` (cite the PR#, flag it weak-intent / lower confidence) rather than an anomaly; reserve `likely-bug` for diffs that nothing merged can explain. Only when the window contains **no merged PRs at all** is the intent genuinely empty — "no visual change expected" — and *any* real change is suspect.
   5. **Test-suite changes:** renamed/added snapshots from suite updates are new baselines, not anomalies. If the test suite lives in a **separate repo** from the app, gather merged-PR intent from *both* repos.
   6. **Caveat — merges ≠ deploys.** The tested environment may lag the branch: a PR merged an hour ago is "expected intent" by the window but may not be deployed to the environment the suite ran against. When the user can provide a deploy window (release tag, deploy manifest, deployment timestamp), prefer it over the merge window.
   Do NOT fall into NO-INTENT mode. Verdicts then run normally: a diff that matches a merged PR's intent → **expected** (cite the PR#); **dynamic/flaky** via Step 6b's history check; the rest → **genuine anomaly** (deploy/data/env — or a regression the PR loop missed) → `likely-bug`, noting the likely-external cause. `/percy:gate` treats regression builds as **triage, not iterate** (see its Case R).

**If none yield usable intent — try auto-backfill BEFORE degrading.** If an open PR exists for the branch (or `git diff origin/<default>...HEAD` is non-empty), the intent isn't missing — it just wasn't captured. Invoke `/percy:visual-intent` in proposal mode right now:

> No intent was captured for this PR — I've drafted one from the diff; accept to get full verdicts.

On accept, continue in **FULL mode on the SAME build** — the build is already processed; intent reconciliation is a read-side operation, so **no rebuild is needed**. NO-INTENT mode is only for genuinely diff-less situations (no PR *and* an empty diff against the default branch — nothing to draft intent from).

**Only then — enter NO-INTENT DEGRADED MODE:**

Without a stated intent, the mechanical rule misflags intentional changes that look like irregularities to Percy's classifier (e.g., A/B test variants, feature toggles, dependency-driven UI changes). The intent layer is what filters these.

In NO-INTENT mode:
- **Do not produce per-comparison `expected | likely-bug | ambiguous` verdicts.** The signal isn't reliable.
- Surface a strong warning at the top of the output:
  > ⚠️ **No Visual Intent found.** This run will report Percy's raw `irregularity` signal without intent reconciliation. Expect false positives on intentional changes (A/B tests, feature toggles, planned redesigns). To get reliable verdicts, run `/percy:visual-intent` first to capture what's *meant* to change.
- For each comparison with any `visual_quality: irregularity` region, output: `🐞 candidate-bug` (not `likely-bug`). The "candidate" label signals to the user that this needs eyeballs because the plugin can't tell if it's real.
- For each comparison without irregularities, output: `🔄 unverified-change` (diff exists, no AI-flag, no intent to compare against).
- The output is informational only — `/percy:gate` will refuse to auto-reject in this mode.

Skip the rest of the verdict matrix below. Jump to Step 10's output.

### 8. Reason per-comparison (the agent does this, not Python)

For each comparison, you have:
- A list of regions Percy detected, each with: `change_title`, `change_description`, `change_reason`, `visual_quality`, `visual_quality_reason`
- The cluster(s) this comparison belongs to, each with a `cluster_title` narrative
- For each cluster, whether it's bug-promoted (Step 6)
- The Visual Intent (intended changes + unchanged invariants)

**Reason as follows, in natural language. The key signals are Percy's `change_reason` and your code context — use them, don't just look at the binary `visual_quality` flag.**

**Signal precedence (apply in this order — do not skip ahead):**
1. **Stated intent + matching code change** — the developer's Visual Intent and whether the git diff actually touches this surface. Strongest, most reliable signal. If intent/code settles it, you're done.
2. **Percy's `change_reason` / `visual_quality`** — Percy's own deliberate-vs-unintended judgment. Use when intent/code is silent on this surface.
3. **General visual coherence** ("the page still looks clean / the change looks deliberate") — a *last-resort tiebreaker only*. **Never clear a flag on coherence alone when intent or code context is available.**

> **Why precedence matters — worked counter-example (illustrative).** A coherence-only pass would clear such a build's flag by reasoning "it's just an icon swap, looks fine" — the *right verdict* (not a bug) for the *wrong reason*. The truth: the developer **intentionally added a voice-input feature**, visible only in the PR/code, not inferable from "it looks coherent." Coherence got lucky here; on a coherent-looking *real* regression it would have failed. Always reach for intent/code first; treat "looks fine" as the weakest evidence.

> **Why "no code match" must NOT clear an `irregularity` — worked counter-example (illustrative).** PR title: *"add font load retry."* Percy flagged thumbnails replaced by spinners / a magenta placeholder as `irregularity` — i.e. **failed asset loads, plausibly the exact bug the PR was trying to fix, still occurring.** An over-eager "no diff touches the thumbnail component → dynamic content → clear" rule cleared all of them. That is the failure mode this gating exists to prevent: code-absence is a strong *clear* signal only when Percy itself said `valid`. When Percy said `irregularity`, code-absence is weak — the defect can be triggered indirectly (asset pipeline, shared dep, infra) — so default to `needs-human`, never auto-clear. *Note: when snapshot history IS available, Step 6b's dynamic-content check resolves this build to `likely-flaky-test` (dynamic carousel) first — see that step. The `needs-human` default here is the no-history fallback.*

> **Reconcile EACH cluster (illustrative) — a matching headline change does not clear the build.** Suppose a PR's stated change (a toast-implementation swap) matches one cluster — an over-eager pass would clear the whole build. But a *second* cluster — a newly added merchant-ID field that reorganised the surrounding layout — was **not in the stated intent and not produced by the toast diff**: an unplanned change the developer slipped in alongside the planned one. Every cluster gets its own verdict: the toast change is `expected`, the merchant-field reflow is `unplanned-change` (flag for human). Never let a matched dominant change suppress a co-occurring unplanned one. (In-session `/percy:visual-intent` would also record the field add, making this explicit.)

> **Intent-match ≠ clean (illustrative).** PR intent: a hidden-password eye-toggle. Percy flagged the eye-toggle region — which *matches the intended surface* — so a naive "matches intent → expected" rule would clear it. But the region was `irregularity` and the change itself was broken. **A cluster that matches the intended surface but whose `visual_quality`/`change_reason` reads regression-like is `likely-bug`, not `expected`** (matrix row: *irregularity · matches intent BUT change_reason unintended → likely-bug*). Matching the surface the dev meant to touch is not evidence the change is correct.

For each region in the comparison:

1. **Read the region's `change_title` + `change_description` + `change_reason`.** `change_reason` is the most valuable field — it's Percy's own natural-language judgment of *whether the change looks deliberate or accidental*. Examples:
   - *"This is a deliberate CSS color change that affects the site's branding/visual hierarchy… not an expected dynamic variation like timestamps"* → Percy itself says **deliberate**.
   - *"could represent a focus/validation state or an unintended CSS regression"* → Percy itself flags **possibly unintended**.
   - *"minor sub-pixel shift"* / *"timestamps differ"* → Percy itself flags **dynamic / benign**.
   Weight `change_reason` heavily — Percy already looked at the pixels and wrote down what it saw.

2. **Cross-reference the code context (Step 7b).** Does a code change in the git diff correspond to this region's surface?
   - **Code change matches the region** (e.g., region says "heading color changed" and the diff edits `h1` CSS) → this is a real, *intentional* change. Reconcile against intent.
   - **No code change touches this surface** → the inference depends on what Percy's classifier said about the region:
     - **If `visual_quality === "valid"`** (Percy saw no defect): "no matching code change" is the **single strongest false-positive filter** — the change is almost certainly **dynamic content / fixture variance / data-driven** (e.g., region says "product list reordered" but no diff touched the list). Clear it.
     - **If `visual_quality === "irregularity"`** (Percy independently saw a quality defect — spinner, broken/missing image, magenta placeholder, truncated/garbled text, layout break): **"no matching code change" does NOT clear it.** Code-absence is a weak signal here because a defect can be triggered *indirectly* (a shared dependency, a config/infra change, a build-time asset pipeline) without a diff hunk on the visible surface. Percy looked at the pixels and flagged a defect — do not overrule that on code-absence alone. Default to **`needs-human`** (Step 9b); escalate to **`likely-bug`** if it also violates an unchanged invariant or the snapshot/PR context makes a regression plausible (e.g. an asset-loading PR + broken-image irregularities).

3. **Compare semantically against the Visual Intent's "intended" list.** Does this region describe one of the intended changes? (Semantic understanding, not keyword match.)

4. **Cross-check against the Visual Intent's "unchanged" list.** Does this region describe a change to something the intent said should stay the same?

5. **Read the snapshot name** for additional framing — e.g. a snapshot named "error page - org not found" tells you the page is *meant* to show an error, so a rendered error state is expected, not a bug.

6. **Combine into a verdict:**

| Region's `visual_quality` | Reconciliation result | Verdict |
|---|---|---|
| `valid` | Matches intent | **expected** |
| `valid` | Not in intent, but `change_reason` says dynamic/benign OR no matching code change | **expected** (benign dynamic content) |
| `valid` | Not in intent, and there IS a matching code change not covered by intent | **unplanned-change** — flag for human |
| `valid` | Violates "unchanged" invariant | **likely-bug** |
| `irregularity` | Matches intent AND `change_reason` reads as deliberate AND a code change supports it | **expected** (Percy flagged a planned change — `change_reason` confirms deliberate) |
| `irregularity` | Matches intent BUT `change_reason` reads as unintended/regression-like | **likely-bug** (intentional surface, but the *way* it changed looks broken) |
| `irregularity` | Not in intent, `change_reason` *explicitly* says dynamic/benign (e.g. "timestamps differ", "sub-pixel shift") | **expected** (dynamic content Percy over-flagged) |
| `irregularity` | Not in intent, no matching code change, but `change_reason` does NOT call it benign | **needs-human** (do NOT clear on code-absence alone — Percy saw a defect; a regression can be triggered indirectly) |
| `irregularity` | Not in intent, no benign explanation | **likely-bug** |
| `irregularity` | Violates "unchanged" invariant | **likely-bug** |

There is no `ambiguous` tier — those cases are **resolved inline here** using `change_reason` + code context. A case that genuinely can't be resolved from text + code becomes `needs-human` (Step 9), not `ambiguous`.

Also factor in the cluster-promoted bug flag: if the cluster is bug-promoted but this specific region was `valid`, treat the region as if `visual_quality === "irregularity"` for verdict purposes (consistency rule), then run the same reconciliation. Note in the output that the bug was promoted from cluster siblings.

**Per-comparison verdict = worst region verdict** (ordered: likely-bug > unplanned-change > needs-human > expected).

**For `unplanned-change`, also set `actionable_unintended` (boolean).** An unplanned-change is a real, code-backed change that isn't in the stated intent — which can be either an *accidental side-effect* or a *deliberate change the developer simply didn't write down*. Decide which, and set the flag so `/percy:gate` knows whether to auto-iterate or defer to a human:
- `actionable_unintended: true` — you're **confident it wasn't intended**: it reads like a ripple/side-effect (e.g. a shared-component or global-CSS edit changed a surface the PR wasn't about), it's regression-adjacent, or it contradicts the spirit of the stated intent. The gate folds these into the iterate set and the agent fixes them — and if the developer says it *was* deliberate, the fix is to add it to intent, not revert.
- `actionable_unintended: false` — it could **plausibly be deliberate-but-unstated** (a reasonable change on a surface the dev was working in, just not mentioned). The gate routes these to human review — don't risk auto-reverting something intended.

When in doubt, prefer `false` (send to a human) — auto-iterating on a deliberate change is worse than asking.

### 9. Asset-capture pre-check + needs-human resolution

**Step 9a — Asset-capture pre-check.** If any region's `change_description` / `change_reason` mentions broken/missing images ("image not loading", "broken image", "missing asset", "image overflow", "alt text showing"), probe Percy's smart-debug data BEFORE finalizing a `likely-bug` verdict:

```
GET /api/v1/network-logs?comparison_id=<id>
Authorization: Basic <base64(PERCY_USERNAME:PERCY_ACCESS_KEY)>
User-Agent: curl/8.7.1
```

The response includes per-resource `asset-captured` status (`ok | NA | error | skipped`) plus failure categories like `asset_capture_failure` or `responsive_asset_capture_network_idle_failed`.

**If asset-capture failures are present for image resources in the diff** → downgrade verdict to `asset-capture-issue` (not a bug):
> "Broken images in this comparison are due to Percy's asset capturer missing them (CDN or network-idle issue), not a real visual regression. See the Snapshot Debug page for the resource list."

Surface the failing resource URLs and a link to Percy's Snapshot Debug page.

**Step 9b — needs-human escape hatch.** A small residual of cases genuinely can't be resolved from Percy text + code context + intent — e.g. a rare one-off flake whose snapshot history is otherwise stable, or a change whose intent is ambiguous even with the PR body. For these:
- Mark the verdict `needs-human`.
- Surface Percy's `change_description` + `visual_quality_reason` verbatim + the dashboard deep-link.
- Do NOT guess. The human spends ~30 seconds; the plugin doesn't misfire.

This is the by-design human fallback. It is the *only* path that hands off to a person, and it is text-driven (no images).

### 10. Output

Render a per-comparison verdict table:

| # | Snapshot | Browser | Width | Cluster(s) | Verdict | Why | Open |
|---|---|---|---|---|---|---|---|
| 1 | `Login page` | Chrome | 1280px | 1 | ✅ expected | Heading color change matches intent; change_reason "deliberate"; diff edits h1 CSS | [open ↗](<snapshot deep-link>) |
| 2 | `Login page` | Firefox | 375px | 1,2 | 🛑 likely-bug | Input shadow not in intent; change_reason "unintended focus style"; diff touches shared Button | [open ↗](<snapshot deep-link>) |
| 3 | `Dashboard` | Chrome | 1280px | 2 | ✅ expected | Product list reordered, but no code change touches the list → dynamic fixture data, not a regression | [open ↗](<snapshot deep-link>) |

URL pattern:
```
<project_base>/builds/<build_id>/changed/<snapshot_id>
```
Where `<project_base>` = first 3 path segments of `web-url` (e.g., `https://percy.io/<org>/web/<project>`).

Percy doesn't deep-link to a specific comparison — only snapshot. Label each row with the browser + width so the user knows which tab to select inside the snapshot view.

**Link formatting — deep, specific, and clickable (applies everywhere the plugin surfaces a link, here and in `/percy:gate`):**
- **Deep-link to the specific snapshot that has the issue** — the `…/changed/<snapshot_id>` URL — **not** the build root. If only one snapshot/comparison is flagged, the link must point straight at that snapshot, not at the build.
- **Always render as a Markdown link** so it's clickable in the IDE — `[<snapshot name> — <browser> @ <width>px](<deep-link>)`, never a bare/angle-bracketed URL. (Claude Code and other IDE chats render Markdown; a raw `<url>` shows as plain text.)
- Reserve the build-root link for genuinely build-wide statements (e.g. "approve the whole build"); anything tied to a specific snapshot gets that snapshot's deep-link.
- **Never wrap user-facing output in code fences.** Verdict tables, summaries, and anything containing a link must be emitted as normal Markdown (tables/blockquotes/lists) — a fenced block renders links as dead text in the IDE. Code fences are for commands and payloads only.

One-line summary:
> Build #42: 0 expected · 1 ambiguous · 11 likely-bug (after cluster promotion) · 0 unplanned-change · 1 raw potential-bug

Return the structured verdict object:
```json
{
  "build_id": "12345678",
  "build_number": 42,
  "build_url": "https://percy.io/.../builds/12345678",
  "ai_coverage": { "total_comparisons": 12, "with_ai": 12, "summary_status": "ok" },
  "clusters": [
    { "idx": 1, "title": "...", "bug_promoted": true, "occurrences": 3 }
  ],
  "comparisons": [
    {
      "cid": "...", "snapshot_name": "...", "browser": "Chrome", "width": 1280,
      "clusters": [1,2],
      "regions": [
        { "title": "...", "visual_quality": "valid", "intent_match": "matches intended #1" }
      ],
      "verdict": "expected",
      "reasoning": "Both regions match intent's 'revert heading rebrand' clause; change_reason reads deliberate; git diff edits the h1 CSS.",
      "code_change_matched": true,
      "actionable_unintended": false,
      "dashboard_url": "<snapshot link>"
    }
  ]
}
```

`/percy:gate` consumes this object.

## Cold start — no approved baseline yet (check BEFORE NO-AI fallback)

A build with no AI is most often **not** a failure — it's a baseline that hasn't been established/approved yet. Percy's AI only runs when there's an **approved baseline to diff against**. Two cold-start cases, both of which otherwise produce a confusing NO-AI result:

**Case A — first build / new branch (this build IS the baseline).**
Signals (any of):
- `GET /builds?filter[branch]=<branch>` returns only this build (no prior build on the branch), OR
- the build's `relationships.base-build.data` is **null** (no base to diff against).

There are no comparisons to classify because nothing came before it. **This is expected, not a bug.** Output:
> 🟢 **First Percy build on this branch — it's the baseline, not a regression check.** There's nothing to diff against yet, so there's no AI verdict to give. Approve this build in Percy to lock it as the baseline; the *next* build will get real visual diffs + AI classification. [Open the build in Percy ↗](<build deep-link>)

**If write creds are verified** (`write_creds_verified: true` in config, or `PERCY_USERNAME` + `PERCY_ACCESS_KEY` in the env), don't stop at the deep-link — offer to do it here:
> Approve it as the baseline now? (y/n)

On yes, `POST /api/v1/reviews` with `action: "approve"` (same request shape + Basic auth as `/percy:gate`'s approve). Whichever path the user takes — in-chat approve or the dashboard link — once the build is approved, update `.percy/config.yml → has_approved_baseline: true` and print:
> Baseline locked. From here: change UI, open a PR — intent, watch, review and gate run automatically.

Do **not** run `/percy:gate` — there's nothing to gate.

**Case B — fresh/upgraded baseline awaiting approval (AI blocked until approved).**
Signals: the build has comparisons (`total-comparisons > 0`) but `total-comparisons-with-ai === 0`, AND `review-state` is `unreviewed` / `total-snapshots-unreviewed > 0`, AND/OR `different-base-build === true` (Percy shifted the base, e.g. a browser-version upgrade created a new baseline). Percy commonly defers AI on a build whose baseline is itself unapproved. Output:
> 🟡 **Baseline needs approval before AI runs.** This build diffs against a new/upgraded baseline that isn't approved yet (`review-state: unreviewed`, `different-base-build: true`), so Percy hasn't produced AI classification. Approve the baseline build in Percy, then re-run `/percy:review` (or re-trigger the build) to get verdicts. [Open the build in Percy ↗](<build deep-link>)

Surface the changed snapshots with deep-links so the user can approve quickly. Only fall through to NO-AI fallback if **neither** cold-start case matches.

## NO-AI fallback mode (Step 3 short-circuit)

If neither cold-start case (above) applies AND `total-comparisons-with-ai === 0` AND summary-reason is NOT `no_ai_changes` (i.e., AI genuinely didn't process this build):

1. List all comparisons with diffs (via `GET /snapshots?build_id=<id>&page[limit]=100`, then filter snapshots with `attributes.diff-ratio > 0` or `attributes.review-state-reason === "unreviewed_comparisons"`).
2. Categorize the cause:
   - `ai-enabled: False` AND no AI on any of the project's recent builds → **likely no-AI tier or org-level disabled**
   - `ai-enabled: False` BUT other recent builds DO have AI → **likely browser pool config issue** (this build's browsers aren't AI-eligible; AI-eligible = Chrome/Firefox on Linux currently)
   - Some recent builds have AI, this one doesn't → **likely browser-upgrade limbo or branch config**
3. Output: list all changed snapshots with dashboard deep-links, label the cause, and tell the user the plugin **cannot auto-classify**. Recommend manual review on Percy dashboard.
4. **Do not call `/percy:gate` for a binary decision** in NO-AI mode. The signal isn't reliable enough.

## Important

- **The agent does the verdict reasoning, not Python.** Resist building keyword/regex matchers for intent ↔ region matching. Use semantic understanding.
- **`change_reason` is the highest-value Percy field.** It's Percy's own deliberate-vs-unintended judgment in plain text. Weight it heavily.
- **Code context is the strongest false-positive filter.** A Percy-flagged region with no matching code change in the git diff is almost always dynamic content / fixture variance, not a regression.
- **Percy's text + code context are the only inputs.** The plugin is Percy-AI-native by design (see "Design principle"). Do not add any further inference pass; unresolvable cases route to `needs-human`.
- **Surface Percy reasoning verbatim** (`change_description`, `change_reason`, `visual_quality_reason`) — these are the most useful signals for both the agent and the human.
- **Cluster-level bug promotion is the ONE mechanical rule.** Apply consistently; don't extend.
- This skill is **read-only** for verdicts. Never call `/reviews` to act on a verdict from here — that's `/percy:gate`'s job. The one exception: the cold-start Case A baseline approve (user-confirmed, nothing to gate).
