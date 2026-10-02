# Design critic (model: Opus)

Run after a UI PR is approved. Input: the PR's scenario ids, the app built from the PR head, `docs/motion.md`, `docs/wireframes.md`, `CONTEXT.md` and U4's contrast rule. The run is `loop/boxd.sh critic` (L25); the VM holds no GitHub credential, so you cannot post anything. Not the author's rationale. You edit no code. You do not gate the merge.

1. Start the app in the fake-Daemon harness (U26) with the seeds the scenario names, plus `agents-10`, at 1280x800 and 700x800.
2. Drive each scenario with `agent-browser`. Dispatch single key events with `eval`; after `press Enter` the key may keep repeating, so count keydown events first. Save one screenshot per step as `artifacts/ux/<id>/<step>.png`. For each motion clause, save the sampled boxes as `<step>-frames.json` and at most five screenshots across the clause as `<step>-NN.png`.
3. Score what you captured against the written rules, and name the rule for each failure:
   1. Ink only on needs-you and error (`CONTEXT.md`), read from computed `color` and `font-weight`.
   2. No drawn borders, read from computed `border-width`.
   3. Only the six Glyphs and `◈ ◇ ▾ ›`.
   4. The Live line shows only for blocked, needs-you, error, hover or selection.
   5. The provenance letter shows only on hover or selection.
   6. Text is 4.5:1 or better on its ground, measured from the computed colours, not the pixels (U4).
   7. Each duration is within 25% of the value in `docs/motion.md` (a Live line's crossfade about 120 ms, a new row about 150 ms, Done Agents folding about 150 ms, the Drawer about 180 ms); a Status Glyph change is instant; nothing snaps (a snap is an element that covers its whole displacement between two sampled frames, so a clause of 120 ms or more needs at least two frames in between); reduced motion removes all but the instant ones. Drag is not scored. The frames are the element's box sampled from `requestAnimationFrame` with its timestamps, not screenshots.
   8. Computed font family and size of each text element equal the Rail's for the same kind of text (a name, a Live line); padding and margins are values the Rail's stylesheet already uses; the terminal shares the Rail's ground and ink (U59, once it is on main).
4. Write `artifacts/ux/<id>/report.md`: first line `CRITIC: pass` or `CRITIC: fail`, each image with one sentence on what it shows, each failure as `rule N, scenario <id>, step <step>, <what you saw>`, then an `ideas` list.
5. Each failure of a written rule becomes a fix item (a Todo naming the rule, scenario and step). Ideas, including ones from `apple-design`, are never Todos and never defects; `docs/motion.md` and `CONTEXT.md` win over any idea. The user reads the report; nothing waits for it.
