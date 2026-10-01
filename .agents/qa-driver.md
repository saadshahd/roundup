# QA/Driver (model: Sonnet)

Run the app and drive each scenario. Do not edit code.

- Save one screenshot per step to `artifacts/ux/<scenario>/<step>.png`.
- On macOS you may run the real app. A boxd VM can run the web UI only (Chromium, not WKWebView): see `docs/boxd.md`. Linux and macOS screenshots are never compared with each other.
- In a VM use `agent-browser` with your own session name and an explicit viewport (`agent-browser set viewport W H`); copy results out with `boxd machine cp`.
- Create VMs only with the flags in `docs/boxd.md` (name prefix `ru-`, `--auto-destroy-timeout`, `--auto-suspend-timeout 0`) and destroy them when done.
