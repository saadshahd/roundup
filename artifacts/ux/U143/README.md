# U143 rendered observer

Base: `5031732` (main). Head: the U143 source in this PR. Chromium on the same Mac, light theme, 1280×800 and 700×800. These captures cover the webview, not the native folder chooser or a real Claude process.

For each build, serve `apps/desktop` with Vite and open `/harness.html?seed=agents-10` with agent-browser. Remove the seed's rows through `__fake.app.rpc("rail.remove", {id})` to obtain an already open, empty Project. Capture `empty`. Click the Rail's `+ room`, then emit `terminal.exited` with code 137 for that Room's Terminal and capture `stopped`. Each `.checks.json` is the complete result of `window.__checks()`.

`loop/rules.sh delta artifacts/ux/U143/base artifacts/ux/U143/head` exits 0; `delta.txt` records all 40 comparisons. No previously passing Check regressed. D1/D2/D7/D8/D10 already fail on these main screens; D6 also fails with a stopped Door. This does not establish that the design system is complete.

The first-Room action and failure/retry behavior are covered by `u143_` tests. Keyboard activation was also exercised with agent-browser at both widths; held RPCs are tested with promises, because the fake answers immediately. No coordination or persisted Room-scoped results are claimed.

| Screen | Before | After |
|---|---|---|
| Empty Project, 1280 | [base](base/empty-1280.png) | [head](head/empty-1280.png) |
| Empty Project, 700 | [base](base/empty-700.png) | [head](head/empty-700.png) |
| Stopped Door, 1280 | [base](base/stopped-1280.png) | [head](head/stopped-1280.png) |
| Stopped Door, 700 | [base](base/stopped-700.png) | [head](head/stopped-700.png) |
