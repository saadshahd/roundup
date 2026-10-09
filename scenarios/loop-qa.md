# Loop QA

Module: `loop/qa-sweep.sh`, `.github/workflows/qa.yml`. Each id's cases are its tests in `loop/qa-sweep.test.sh` (L52) and `loop/runs.test.sh` (L66), which the `rules` job of `.github/workflows/loop.yml` runs.

**L52 sweep.** `qa-sweep.sh` drives `just harness` in `agent-browser` at 640×400 and 1280×800 with each Drawer opened and closed, and writes `loop/out/qa/sweep-<UTC>.json`: `{head, started, complete, viewports, scenarios, findings}`, each finding `{id, text, at, scenario, test}`; `complete` is true only when the sweep reached its end, findings or not.

**L66 scheduled QA.** Every six hours `qa.yml` sweeps `main` with L52 on a fresh runner holding no write token, unless a `qa` run already swept that SHA to the end (`runs.sh swept`: its step `a complete sweep record` passed, which needs a record with `complete` true; a run that failed before or during the sweep, or was cancelled, does not count, and a `gh` failure exits 4). A finding fails the run; the sweep record is its artifact.
