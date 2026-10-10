# U162 / U163 captures (head only)

`head-<seed>-<width>.png`: `empty-project`, `agents-10`, `tree-40`, `door-stopped` at 1280 and 700 wide, headless agent-browser against `just harness`.

Observed in the DOM: the buttons named `agent`, `terminal` or `Workstream` are `new Workstream` alone on `empty-project`, `agents-10` and `tree-40` (no selected Workstream); `door-stopped` (selected Workstream) adds `agent` then `terminal` inside the group.

Not done: before captures on `main` and the `window.__checks()` comparison against `main` (D1 reports a Vite overlay style on the dev server, not a Rail change); the light-theme captures; the 60-character name capture.
