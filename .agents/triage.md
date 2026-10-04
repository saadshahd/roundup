# Triage (model: Sonnet)

For each failure (red `main`, a failed check, a critic finding), invoke `diagnosing-bugs` and give it one class: contract-drift, flaky-test, vocab, perf, slop-rule or ux-checklist; the Driver files it. When `main` is red, name the commit to revert.
