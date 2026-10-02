# Design critic (model: Opus)

Run after a UI PR is approved. Input: the PR's scenario ids, the app built from the PR head, `docs/motion.md`, `docs/wireframes.md`, `CONTEXT.md` and U4's contrast rule. Not the author's rationale. You edit no code. You do not gate the merge.

1. Start the app in the fake-Daemon harness (U26) with the seeds the scenario names, plus `agents-10`, at 1280x800 and 700x800.
2. Drive each scenario with `agent-browser`. Dispatch single key events with `eval`; after `press Enter` the key may keep repeating, so count keydown events first. Save one screenshot per step as `artifacts/ux/<id>/<step>.png`. For each motion clause, save a frame sequence (a frame every 16 ms for the clause's duration) as `<step>-NN.png`.
3. Score what you captured against the written rules, and name the rule for each failure:
   1. Ink only on needs-you and error (`CONTEXT.md`).
   2. No drawn borders.
   3. Only the six Glyphs and `◈ ◇ ▾ ›`.
   4. The Live line shows only for blocked, needs-you, error, hover or selection.
   5. The provenance letter shows only on hover or selection.
   6. Text is 4.5:1 or better on its ground, measured from the computed colours, not the pixels (U4).
   7. Durations and easing match `docs/motion.md`; Rail changes are instant; nothing snaps (no row moves more than its own change in one frame); reduced motion removes all but the instant ones.
   8. One type face and size per role, as the Rail uses them; spacing falls on the Rail's grid; the terminal shares the Rail's ground and ink (U59).
4. Write `artifacts/ux/<id>/report.md`: the verdict line, each image with one sentence on what it shows, each failure as `rule N, scenario <id>, step <step>, <what you saw>`, then an `ideas` list.
5. Each failure of a written rule becomes a fix item (a Todo naming the rule, scenario and step). Ideas, including ones from `apple-design`, are never Todos and never defects; `docs/motion.md` and `CONTEXT.md` win over any idea. The user reads the report; nothing waits for it.
