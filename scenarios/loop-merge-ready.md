# Trusted merge-ready

Module: `loop/`, `.github/workflows/merge-ready.yml`; ids L78.

**L78 disarm auto-merge only when enabled.** Given a PR that changes `.github/`, when the trusted merge-ready workflow computes its status, then it reads `autoMergeRequest` before attempting to disarm auto-merge. A null request skips disarming and continues through the existing merge-ready gates; an enabled request is disarmed before those gates run. A GitHub read error or a failed disarm publishes a failure status, exits nonzero and never runs the gates. A PR outside `.github/` neither reads nor changes auto-merge. The regression runs the workflow's shell with fake `gh` for each case, verifying the calls, order and final status without GitHub access. This serves P5 by preserving the landing gates without reporting an absent auto-merge request as a failure.
