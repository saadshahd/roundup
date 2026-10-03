# U111 proof: scrollLeft of `.columns` during the slide

Captured on `ru-u111` (Linux boxd VM), Chromium via `agent-browser`, against `just harness tree-40`
on `http://localhost:5199`. `.columns`' `scrollLeft` was sampled every 10ms with
`setInterval` across the open and close transitions (each window covers the 180ms
`DRAWER_SLIDE_MS` transition plus slack).

Note: this is a Linux/Chromium repro environment, not the macOS WKWebView the bug was found in.
Per the scenario (`scenarios/ui-drawer.md` U111), Chrome already returned `scrollLeft` to 0 even
before the fix ("stays there in the real WKWebView window (800 by 600) though Chrome returns it to
0"), so a zero reading here does not by itself prove the WKWebView regression is fixed — the jsdom
unit tests (`drawer/u111_layout.test.tsx`, written first by architect-c)
are what pin the `overflow-x: clip` and `{ preventScroll: true }` behaviour the fix relies on. These
screenshots and samples demonstrate the layout stays put and `overflow-x` computes to `clip` for a
real render, for both Drawer kinds and both named window sizes.

| Viewport | Drawer | Transition | Samples | min scrollLeft | max scrollLeft |
|---|---|---|---|---|---|
| 1600x1200 | Todo | open  | 50  | 0 | 0 |
| 1600x1200 | Pad   | open  | 482 (recorder left running past the close click; interval not cleared in time) | 0 | 0 |
| 800x600   | Todo  | open  | 38  | 0 | 0 |
| 800x600   | Todo  | close | 41  | 0 | 0 |
| 800x600   | Pad   | open  | 41  | 0 | 0 |
| 800x600   | Pad   | close | 39  | 0 | 0 |

Screenshots (closed-before / open-after / closed-after) for each combination are alongside this file:
`todo-1600x1200-*.png`, `pad-1600x1200-*.png`, `todo-800x600-*.png`, `pad-800x600-*.png`.
