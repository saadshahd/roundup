#!/usr/bin/env bash
# L2, L23: stamp authored commits in a disposable Builder checkout before the model runs.
set -euo pipefail
[[ ${1:-} =~ ^[A-Za-z0-9_.-]+$ ]] || { echo 'expected an Author-Agent id' >&2; exit 2; }
: "${RUNNER_TEMP:?requires a disposable runner directory}"
hooks=$(mktemp -d "$RUNNER_TEMP/roundup-author.XXXXXX")
printf '#!/usr/bin/env bash\nset -euo pipefail\ngit interpret-trailers --in-place --if-exists doNothing --trailer %q "$1"\n' "Author-Agent: $1" >"$hooks/prepare-commit-msg"
chmod +x "$hooks/prepare-commit-msg"
git config --local core.hooksPath "$hooks"
