# J1 journey audit

Main: `84599c3`. Open PRs: #354 (H18), #352 (B1–B18), #349 (U134, U135), #330 (F7 re-run). Chromium on the same Mac, light theme, `just harness <seed> 5199` (Vite on the fake Daemon), agent-browser, 1280×800 and 700×800. `<stage>-<width>.png` and `.checks.json` (the complete `window.__checks()`) are in this directory. Seeds: `first-run`, `empty-project`, `door-stopped`, `conflict` (Enter), `agents-10` (Understand). Nothing here ran a real Daemon, a real `claude` or the native App.

D3, D4, D5, D8 and D9 pass on every capture. D1 and D10 fail on all ten, D2 on eight (all but `first-run`), D6 and D7 fail only on `door-stopped`. These failures predate this audit; none is introduced or excused here.

| Stage | Observed | Verdict |
|---|---|---|
| Enter | `first-run` offers `choose folder…`; `empty-project` and `door-stopped` carry U143's first action and Door state (its `u143_` tests are on main). Stopped Door fails D6 and D7 at both widths. The native folder chooser and a real Door launch are unobserved. | Unproved: no Daemon-backed run from `first-run` to a Thread. |
| Request | No rendered or Daemon observer asks a Door for one action and reads the resulting Todo and outcome. The only way in is the fake App; real `claude` interaction is F7's open re-run (#330). | Unproved. |
| Understand | `agents-10`: error, needs-you, blocked and working rows differ by glyph and word, the header says `4 need you`, and the centre says `select an agent or a terminal`. The ten-Agent screen fails D1 and D2; no Room is selected in this seed, so selected-Room identification is unobserved. | Unproved: selected Room and routine-progress silence are not observed. |
| Intervene | The seeds show needs-you rows but no Decision Card: U113 is waiting on H18 (#354, in flight), so the user cannot answer anything in the App. | Unproved; owned by H18 then U113. |
| Review | No Room-scoped result view; reopen is covered by the Daemon's A8 and S5 tests, not by a rendered observer. | Unproved. |

## First unproved behavior

Earlier-run Agent or Terminal with no saved output: selecting one leaves a blank pane with no explanation (a Review/Intervene dead end), already specified as **U83** (`scenarios/ui-surfaces.md`, ready, no PR, no Claim). It is the only independent gap with an owner-less ready row; H18/U113 (Intervene) and B1–B18 are owned by open PRs, and F7 by #330. No new implementation row is added. J2 waits on U83.
