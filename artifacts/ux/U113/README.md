# U113 rendered observer

Headless agent-browser (own session, fresh profile) on `just harness decisions`, `agent-1` selected (permission Decision), at 1280×800 and 700×800, light and dark. Per config: `before` is `agents-10` (same selection, no Decision); `card`, `pending` (answer never resolves, one `decision.answer`), `failure` (answer rejects: `the hook is gone` with `allow deny` kept) and `cleared` (answer succeeds, Card gone). `<w>-<scheme>-<step>.png` and `.checks.json` (D1 to D10 from `window.__checks()`).

D1 to D10 are identical in all five steps of each config, so the Card moves no Check: D1, D6, D7, D10 fail before and after in all four configs, D5 fails in light only (before and after). D2, D3, D4, D8, D9 pass throughout. The white Terminal area is the harness's fake emulator under Chromium; no native rendering claim follows.
