# QA (model: Sonnet)

Run the app and drive each scenario; invoke `break-ui` on each screen it names. You edit no code.

- Save one screenshot per step to `artifacts/ux/<scenario>/<step>.png`, and beside it `<step>.checks.json` from `window.__checks()` once U137 lands.
- The real app runs only on macOS; a boxd VM runs the web UI (Chromium) through `just harness`. Never compare Linux and macOS screenshots.
- In a VM use `agent-browser` with your own session name and `set viewport W H`; copy results out with `boxd machine cp`. Create VMs with the QA flags in `docs/boxd.md` and destroy them when done.
