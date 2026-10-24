# R23 host attempt: 40133799182

Exact-model qualification failed: this Builder is `AMD EPYC 7763 64-Core Processor`,
not `AMD EPYC 9V45 96-Core Processor`. #3764 remains open and #3760 remains
blocked by it. Remaining measurement and repair work stays under #3720.

| Evidence | Result |
|---|---|
| [machine.txt](machine.txt) | Identity commands, outputs and statuses; Ubuntu 24.04, x86_64, four online/available CPUs, affinity `0-3`, AMD 7763 |
| [qualification.txt](qualification.txt) | Previous standalone preflight reused unchanged; exact-model mismatch, exit 2 |
| [host-search.txt](host-search.txt) | Historical 9V45 evidence, active Builder jobs, runner inventory denial and execution-facility inspection |

Inspection checkout: `a87bbeffc8618aa4e54e6823709ed0c63823d171`.
Execution: [run 40133799182](https://github.com/saadshahd/roundup/actions/runs/40133799182),
job `120991179949` (`build #3764 / build`), runner `GitHub Actions 1000193781`.
`machine.txt` retains the boot id identifying this VM lifetime.

| Failed command | Failure | Actual exit |
|---|---|---|
| Preflight in `qualification.txt` | AMD 7763; requires exact 9V45 | 2 |
| `gh api repos/saadshahd/roundup/actions/runners` | HTTP 403; resource not accessible by integration | 1 |
| `gh api repos/saadshahd/roundup/actions/jobs/120991179834/logs` | Active sibling Builder; HTTP 404, BlobNotFound | 1 |
| `gh api repos/saadshahd/roundup/actions/jobs/120990756165/logs` | Another active Builder; HTTP 404, BlobNotFound | 1 |

No reachable qualified lifetime was identified. Historical 9V45 reports predate
R23's candidate and provide no live execution endpoint. Active job metadata gives
runner names and labels, not CPU identities or a remote command channel. Inventory
access denial does not establish that no qualifying hardware exists. The existing
Builder facility selects `ubuntu-latest` with no exact-model input and serializes
builds of this Issue, so dispatching another build cannot hand off this lifetime.
No resource was created or execution/selection machinery changed.

These statuses describe inspection and standalone preflight, not perf observer
results. No release measurement build, timing invocation, diagnostic profile or
calibration was attempted; there are no measurement executable hashes or release
settings for this attempt. Lint/test binaries supply no R23 evidence.

The same-lifetime reproduction instructions remain in `docs/perf.md`, heading
“R23 execution lifetime (#3764)”. A qualified Builder must retain all six timing
invocations and both profiles before its lifetime ends; this failed attempt
satisfies none of that measurement acceptance. #3760 retains probe compatibility,
paired measurements, attribution and per-metric conclusions. No Mac observation
or product decision is needed.
