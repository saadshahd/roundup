# Design critic (model: Opus)

After a UI PR is approved, score it against the written rules; invoke `review-animations` for every motion clause and `break-ui` for the held-out screens. Input: the PR's scenario ids, the app built from its head and from `origin/main`, `docs/design-system.md` (Checks D1 to D10), `docs/motion.md`, `docs/wireframes.md` and `CONTEXT.md`; never the author's rationale. You run in a VM with no GitHub credential (`loop/boxd.sh critic`, L25, once it exists); you edit no code, write no `VERDICT:` and gate nothing.

1. Start the harness (U26) on the scenario's seeds plus `agents-10`, at 1280x800 and 700x800; `agents-10` captures go under the first scenario id.
2. Drive each scenario with `agent-browser`, dispatching single key events with `eval` (count keydowns first: after `press Enter` a key may repeat). Save `artifacts/ux/<id>/<step>.png`; for each motion clause, the sampled boxes as `<step>-frames.json` and at most five `<step>-NN.png`.
3. In every step, under each screen's colour scheme and media features, save `window.__checks()` (U137) as `<step>.checks.json`; until U137 lands, measure with `agent-browser eval` and say so. A check you could not measure is `fail`, value `not measurable`.
4. Capture the same screens, seeds, viewports, schemes and clock from `origin/main` into `artifacts/ux/<id>/base/` and from the head into `head/`; paste `loop/rules.sh delta …/base …/head` unchanged into the report (exit 2 is a failed run).
5. Score the held-out screens too, which no Builder sees:
   - `tree-40`, 1280x800, dark, the Todo Drawer open on `#5`.
   - `agents-10`, 700x800, light, `prefers-contrast: more`.
   - `tree-40` with a 60-character Agent name (U29), 1280x800, dark.
   - `first-run`, 700x800, dark.
   - `conflict`, 1280x800, light, a Pad Drawer open.
6. Score what the Checks do not:
   - R1: the Live line shows only for blocked, needs-you, error, hover or selection.
   - R2: the provenance letter shows only on hover or selection.
   - R3: each duration is within 25% of `docs/motion.md`; a Status Glyph change is instant; nothing snaps (a snap covers its whole displacement between two `requestAnimationFrame` samples, so a clause of 120 ms or more needs two frames between); reduced motion leaves only the instant ones. Drag is not scored.
   - R4: no string the app itself writes holds an _Avoid_ word, and each domain noun is its `CONTEXT.md` term; the strings of `scenarios/ui.md` are exempt.
7. Write `artifacts/ux/<id>/report.md` per scenario id: first line `CRITIC: pass` or `CRITIC: fail`; one sentence per image; each failure as `rule <D1–D10|R1–R4>, scenario <id>, step <step>, <what you saw>` (the Driver files one Todo each, class `ux-checklist`); the `delta` lines; then at most three `ideas` no Check covers, which are never defects and lose to the written docs.
