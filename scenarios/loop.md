# Loop tooling

**L1 size.** Given a PR, when `loop/rules.sh size` runs, then it prints an advisory on stderr, and still exits 0, when the changed files span more than one module directory or exceed about 2000 changed lines (lockfiles and files under `generated/` excluded); otherwise it prints nothing. Neither guide ever makes it fail. A binary file and a rename into another module each count toward the module list. An unknown base ref fails loudly.

**L2 approval.** Given commits that each carry `Author-Agent`, and an empty newest commit carrying only `Reviewed-by-Agent`, when `loop/rules.sh trailers` runs, then it passes. It fails when the reviewer id is also an author id, when a commit that changes files carries the reviewer trailer, when a commit follows the approval, and when a commit has neither trailer.

**L3 vocabulary.** Given a public Rust item or field, TS export or `contracts/` name containing an `_Avoid:_` word from `CONTEXT.md` (also as a plural), when `loop/rules.sh vocab` runs, then it fails and names the word. `crates/agents/claude_code/` is exempt. A missing `CONTEXT.md` fails loudly.

**L4 pause.** Given Builder output with `api_error_status` 429 or "usage limit" / "rate limit" text, when `loop/boxd.sh build` runs, then it exits 75, writes `loop/out/PAUSED` and keeps its first timestamp on later runs. While the file exists no VM is created.

**L5 cleanup.** Given any exit path of `loop/boxd.sh build`, then the VM it created is destroyed, the original exit code is kept, and a failed removal is reported as a leaked VM. A failed create removes nothing. A successful run leaves a patch in `loop/out/patches/`.

**L6 secret.** Given the boxd secret `CLAUDE_CODE_OAUTH_TOKEN` is missing, when `loop/boxd.sh build` or `review` runs, then no VM is created and it exits 1. A VM only ever sees the secret's placeholder, so no token is scanned for or handled.

**L7 isolation.** Given a Builder run, then its VM is created from the `ru-toolchain` snapshot with `--isolated`.

**L8 cap.** Given `BOXD_MAX_VMS` `ru-` VMs (default 12), or a held `loop/out/lock`, when `loop/boxd.sh build` runs, then no VM is created and it exits 1. A non-numeric `BOXD_MAX_VMS` exits 2.

**L9 review.** Given a ref, when `loop/boxd.sh review <name> <prompt-file> [ref]` runs, then an isolated VM holds a git checkout whose tag `base` is the merge-base with `origin/main` and whose HEAD is the ref, the Reviewer's answer is written to `loop/out/verdicts/<name>.md`, and the VM is destroyed.

**L10 input.** Given a name outside `^[a-z0-9][a-z0-9-]*$`, a prompt file starting with `-`, or a ref starting with `-`, when `build` or `review` runs, then it exits 2 before creating a VM or a file.

**L11 reboot.** Given a VM created from the snapshot, then it is rebooted and answers `exec` before any agent or check runs on it.

**L12 check.** Given a PR number or a branch name, when `loop/boxd.sh check` runs, then it fetches `origin/main` and that ref, merges them in the object store on this machine, uploads that merged tree (tag `base` is `origin/main`) to a fresh `--isolated` VM with an auto-destroy timer (no GitHub login; nothing is cloned or fetched on the VM; no Claude secret is needed), runs `just check` there from the lockfile, shows the last 25 lines with GitHub token shapes masked, destroys the VM and exits with the check's exit code. A ref with shell syntax or a leading dash fails with exit 2, and a ref that conflicts with `origin/main` fails, both before any VM exists. At `BOXD_MAX_VMS` `ru-` VMs it creates none. `loop/boxd.sh bake` runs on an isolated VM, warms the dependency build from the lockfile and refuses to save a snapshot when its scan finds a GitHub or Anthropic token.
