# Loop tooling

**L1 size.** Given a PR, when `loop/rules.sh size` runs, then it prints an advisory on stderr, and still exits 0, when the changed files span more than one module directory or exceed about 2000 changed lines (lockfiles and files under `generated/` excluded); otherwise it prints nothing. Neither guide ever makes it fail. A binary file and a rename into another module each count toward the module list. An unknown base ref fails loudly.

**L2 approval.** Given commits that each carry `Author-Agent`, and an empty newest commit carrying only `Reviewed-by-Agent`, when `loop/rules.sh trailers` runs, then it passes. It fails when the reviewer id is also an author id, when a commit that changes files carries the reviewer trailer, when a commit follows the approval, and when a commit has neither trailer.

**L3 vocabulary.** Given a public Rust item or field, TS export or `contracts/` name containing an `_Avoid:_` word from `CONTEXT.md` (also as a plural), when `loop/rules.sh vocab` runs, then it fails and names the word. `crates/agents/claude_code/` is exempt. A missing `CONTEXT.md` fails loudly.

**L4 pause.** Given Builder output with `api_error_status` 429 or "usage limit" / "rate limit" text, when `loop/boxd.sh build` runs, then it exits 75, writes `loop/out/PAUSED` and keeps its first timestamp on later runs. While the file exists no VM is created.

**L5 cleanup.** Given any exit path of `loop/boxd.sh build`, then the VM it created is destroyed, the original exit code is kept, and a failed removal is reported as a leaked VM. A failed create removes nothing. A successful run leaves a patch in `loop/out/patches/`.

**L6 token.** Given the Claude token in the Builder's result, in the check output or in the Builder's tree, when `loop/boxd.sh build` runs, then it exits 1, deletes the artifact and prints no token. No patch is written for a tree that contains it.

**L7 isolation.** Given a Builder run, then its VM is created from the `ru-toolchain` snapshot with `--isolated`.

**L8 cap.** Given four `ru-` VMs, or a held `loop/out/lock`, when `loop/boxd.sh build` runs, then no VM is created and it exits 1.

**L12 check.** Given a PR number or a branch name, when `loop/boxd.sh check` runs, then it fetches `origin/main` and that ref, merges them in the object store on this machine, uploads that merged tree (tag `base` is `origin/main`) to a fresh `--isolated` VM with an auto-destroy timer (no GitHub login; nothing is cloned or fetched on the VM; no Claude secret is needed), runs `just check` there from the lockfile, shows the last 25 lines with GitHub token shapes masked, destroys the VM and exits with the check's exit code. A ref with shell syntax or a leading dash fails with exit 2, and a ref that conflicts with `origin/main` fails, both before any VM exists. At four `ru-` VMs it creates none. `loop/boxd.sh bake` runs on an isolated VM, warms the dependency build from the lockfile and refuses to save a snapshot when its scan finds a GitHub or Anthropic token.

