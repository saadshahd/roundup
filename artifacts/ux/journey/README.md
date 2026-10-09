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

# J2 journey audit

Main: `4649c5f` (holds U83, H18, U143; open PRs #393 U113, #396 B1–B18, #349 U134/U135, #330 F7 re-run). Same method as J1: `just harness <seed> 5199`, agent-browser, Chromium, light, 1280×800 and 700×800, all twelve seeds-by-width read with `window.__checks()`. New captures: `j2-earlier-run-*` and `j2-agents-10-*` (`.png` and `.checks.json`). The J1 captures of `first-run`, `empty-project`, `door-stopped` and `conflict` were repeated and read the same: `first-run` fails D1, D10; `empty-project` and `conflict` fail D1, D2, D10; `door-stopped` additionally fails D6 and D7 (`textarea.xterm-helper-textarea`: no hover state, no pressed state) at both widths. Nothing here ran a real Daemon, a real `claude` or the native App.

| Stage | Observed | Verdict |
|---|---|---|
| Enter | Unchanged since J1. The webview reaches a Daemon only through Tauri commands, so no browser run can go from `first-run` to a Thread on a real Daemon. | Unproved: needs U144. |
| Request | Daemon half: `b24_` and `d3_` in `crates/rup/tests/e2e.rs` drive a Door over MCP against the fake `claude`; no rendered run reads the Todo in the Shelf. Real `claude` is F7's re-run (#330). | Unproved: needs U144 for the rendered half. |
| Understand | `agents-10` unchanged: glyph and word separate error, needs-you, blocked, working; header `4 need you`; no Room selected, so selected-Room identification stays unobserved. | Unproved. |
| Intervene | No Decision Card renders on main; U113 (#393) is in flight. | Unproved; owned by U113. |
| Review | U83 now renders: `earlier-run` selected shows `no output kept from an earlier run` at both widths, so that dead end is closed. No result view of a Room and no rendered reopen. | Unproved: needs U144 for reopen; Room-scoped results have no contract. |

## Gap selected

Enter, Request and Review share one missing observer: the App cannot be rendered on a real Daemon outside the native App. Specified as **U144** (`scenarios/ui-first-journey.md`), After U143 (done), so ready. Intervene stays with U113 and the Meta-agent spike with #330; neither is duplicated. **J3** waits on U144 and repeats this audit through `just harness-real`.

Loop note, not changed here: `loop/rules.sh ready` marks a policy row done only by a `j<n>_` test, so J1 and J2 print `ready` after merging.
