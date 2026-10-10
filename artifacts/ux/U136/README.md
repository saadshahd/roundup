# U136 baseline

Base: `3541c8a7^` (main before U130's `tokens.css`), with `testing/checks.ts`, `contrast.ts` and `lookLiterals.ts` from `origin/main` (7d225b31) copied in, an empty `tokens.css` and `window.__checks` wired in `testing/harness.tsx`, because base has no D-checks. Head: `origin/main` 7d225b31. Chromium headless on Linux through `just harness` seeds `first-run`, `agents-10`, `tree-40`, `daemon-exits`, `conflict`, at 1280x800, light and dark. 700x800 was not captured. Linux only.

`loop/rules.sh delta base head` exits 0 (`delta.txt`, 100 comparisons): 54 `fixed`, 22 `still-passing`, 24 `still-failing`, **0 `regressed`**.

## Not met: the finish line is not reached in a real browser

On head, the live `window.__checks()` fails:

- D1 on every seed: the document holds colour, shadow or duration literals (10 on `tree-40`), the first from xterm's own stylesheet (`box-shadow: var(--vscode-scrollbar-shadow, #000)`, `transition: opacity 100ms linear`), which the `u136_` jsdom run does not load.
- D2 on `tree-40`: `div.rail-row[data-id="auth"]` has `padding-left: 16.542px`, off the space grid.
- D6 on `tree-40`: `button.todo-row-button` has `cursor: grab`.
- D10 is the aggregate of the above.

So U136 stays open: "D1 to D10 pass" holds in `u136_` but not on the rendered page. Each failure needs its own work Issue.
