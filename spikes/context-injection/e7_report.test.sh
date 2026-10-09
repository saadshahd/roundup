#!/usr/bin/env bash
# E7: the report records, for each of the three seams, whether the model could state its word
# or the server started before the first prompt, and the version of `claude` it ran on.
# Usage: spikes/context-injection/e7_report.test.sh
set -euo pipefail

dir=$(cd "$(dirname "$0")" && pwd)
report=$dir/REPORT.md

e7_report_names_the_claude_version() {
  grep -Eq 'Claude Code \*\*[0-9]+\.[0-9]+\.[0-9]+\*\*' "$report"
}

e7_report_answers_each_seam() {
  grep -Eq '^\| 1 \| `--append-system-prompt-file`.*could state its word' "$report" &&
    grep -Eq '^\| 2 \| `SessionStart` hook.*could state its word' "$report" &&
    grep -Eq '^\| 3 \| `--mcp-config`.*before the first prompt' "$report"
}

e7_spike_script_and_stub_exist() {
  [ -x "$dir/run.sh" ] && [ -x "$dir/mcp_stub.js" ]
}

for t in e7_report_names_the_claude_version e7_report_answers_each_seam e7_spike_script_and_stub_exist; do
  "$t" || { echo "FAIL $t" >&2; exit 1; }
  echo "ok $t"
done
