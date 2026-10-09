# Loop QA

Module: `loop/qa-sweep.sh`, `.github/workflows/qa.yml`. L52's cases are its tests in `loop/qa-sweep.test.sh`, which the `rules` job of `.github/workflows/loop.yml` runs; only the live schedule observes L66.

**L52 sweep.** `qa-sweep.sh` drives `just harness` in `agent-browser` at 640×400 and 1280×800 with each Drawer opened and closed, and writes `loop/out/qa/sweep-<UTC>.json`: `{head, started, complete, viewports, scenarios, findings}`, each finding `{id, text, at, scenario, test}`; `complete` is true only when the sweep reached its end, findings or not.

**L66 scheduled QA.** Every six hours `qa.yml` sweeps `main` with L52 on a fresh runner holding no write token. It calls no model, so it sweeps an unchanged `main` again. Its step `a complete sweep record` passes only when a record has `complete` true. A finding fails the run; the sweep record is its artifact.
