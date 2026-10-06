# Loop QA

Module: `loop/qa-sweep.sh`, `loop/qa-coordinator*.ts`. Each id's cases are its tests in `loop/qa-sweep.test.sh`, which the `rules` job of `.github/workflows/loop.yml` runs, and `loop/qa-coordinator.test.ts`; the runbook is `docs/qa-coordinator.md`.

**L52 sweep.** `qa-sweep.sh` drives `just harness` in `agent-browser` at 640×400 and 1280×800 with each Drawer opened and closed, and writes `loop/out/qa/sweep-<UTC>.json`: `{head, started, viewports, scenarios, findings}`, each finding `{id, text, at, scenario, test}`.

**L66 scheduled QA.** Every 10 minutes the coordinator sweeps one exact `main` SHA in one disposable isolated `ru-qa-scheduled` VM with no GitHub login; a late tick, a stale lease, missing output, a timeout or a failed removal stays red until a human acknowledges it.
