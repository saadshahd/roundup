export type CommandResult = { kind: "ok"; output: string } | { kind: "failed" | "unknown"; detail: string };

type Record = {
  head: string | null;
  started: string;
  ended: string | null;
  status: "running" | "passed" | "failed";
  output: string;
  cleanup: "not-created" | "pending" | "removed" | "unknown";
};

export type State = { due: number; lease: number | null; latest: Record | null; failure: string | null };

type Edge = {
  now(): number;
  command(args: string[], timeoutMs: number, output: string): Promise<CommandResult>;
  record(state: State): void;
};

const vm = "ru-qa-scheduled";

export const interval = 600_000;

const leaseLimit = 500_000;

const action = "Stop the Run job; inspect its logs and output; confirm ru-qa-scheduled is absent, then recover and register again (docs/qa-coordinator.md).";

export function inspect(state: Pick<State, "due" | "lease" | "failure">, now: number): string | null {
  if (state.failure) return state.failure;

  if (state.lease !== null && now - state.lease > leaseLimit) return `stale lease: ${action}`;

  if (now > state.due + 60_000) return `missed QA tick due ${new Date(state.due).toISOString()}: ${action}`;

  return null;
}

function detail(result: CommandResult): string | null {
  return result.kind === "ok" ? null : `${result.kind}: ${result.detail}`;
}

async function copyOutput(state: State, edge: Edge, output: string): Promise<void> {
  const copied = await edge.command(["boxd", "machine", "cp", `${vm}:/tmp/roundup-qa-output.tgz`, `${output}/sweep.tgz`], 30_000, `${output}/copy.log`);
  const copyError = detail(copied);

  if (copyError) state.failure = `${state.failure ?? ""} Output copy ${copyError}. ${action}`;
  else {
    const outputCheck = await edge.command(["test", "-s", `${output}/sweep.tgz`], 5000, `${output}/output.log`);
    const outputError = detail(outputCheck);

    if (outputError) state.failure = `${state.failure ?? ""} Missing sweep archive ${outputError}. ${action}`;
  }
}

export async function coordinate(state: State, edge: Edge, repo: string, out: string, maxVMs = 12): Promise<void> {
  const now = edge.now();
  const problem = inspect(state, now);

  if (problem) {
    if (!state.failure && state.lease === null) {
      state.latest = { head: null, started: new Date(now).toISOString(), ended: new Date(now).toISOString(), status: "failed", output: out, cleanup: "not-created" };
    }

    state.failure = problem;

    if (state.lease !== null && now - state.lease > leaseLimit) {
      const output = state.latest?.output ?? out;
      await copyOutput(state, edge, output);
      const removed = await edge.command(["boxd", "machine", "remove", vm, "--confirm", "--json"], 30_000, `${output}/cleanup.log`);
      const cleanup = removed.kind === "ok" ? "removed" : "unknown";

      if (state.latest) state.latest = { ...state.latest, ended: new Date(edge.now()).toISOString(), status: "failed", cleanup };

      if (cleanup === "removed") state.lease = null;
    }

    edge.record(state);

    return;
  }

  if (state.lease !== null) return;
  state.lease = now;
  state.due = now + interval;
  const output = `${out}/${new Date(now).toISOString().replaceAll(":", "-")}`;
  let record: Record = { head: null, started: new Date(now).toISOString(), ended: null, status: "running", output, cleanup: "not-created" };
  state.latest = record;
  edge.record(state);
  const deadline = now + 420_000;

  async function command(args: string[]): Promise<CommandResult> {
    const remaining = deadline - edge.now();

    if (remaining <= 0) return { kind: "unknown", detail: "420-second QA deadline exceeded" };

    return edge.command(args, remaining, `${output}/commands.log`);
  }

  function hasFailed(result: CommandResult): boolean {
    const error = detail(result);

    if (!error) return false;
    state.failure = `${error}. ${action}`;

    return true;
  }

  try {
    const main = await command(["curl", "--fail", "--silent", "--show-error", "--max-time", "20", "-H", "Accept: application/vnd.github.sha", `https://api.github.com/repos/${repo}/commits/main`]);

    if (hasFailed(main) || main.kind !== "ok") return;
    const head = main.output.trim();

    if (!/^[a-f0-9]{40}$/.test(head)) {
      state.failure = `GitHub returned no exact main SHA. ${action}`;

      return;
    }

    record = { ...record, head };
    state.latest = record;
    edge.record(state);
    const slots = await command(["bash", "-o", "pipefail", "-c", `test ! -e "$1" && boxd machine list --json | jq -er 'if any(.[]; .name == "${vm}") then error("QA VM already exists") else [.[] | select(.name | startswith("ru-"))] | length end'`, "qa-capacity", `${out}/../PAUSED`]);

    if (hasFailed(slots) || slots.kind !== "ok") return;

    if (!/^\d+\s*$/.test(slots.output) || Number(slots.output) >= maxVMs) {
      state.failure = `VM limit reached or invalid list; inspect boxd machine list. ${action}`;

      return;
    }

    // Creation may succeed remotely after the SDK loses its reply, so cleanup starts before creation.
    record = { ...record, cleanup: "pending" };
    state.latest = record;
    edge.record(state);

    if (hasFailed(await command(["bun", `${import.meta.dir}/qa-coordinator.run.ts`, "create"]))) return;

    const script = `set -eu
. "$HOME/.cargo/env"
mkdir -p /tmp/roundup-qa-output
trap 'tar -czf /tmp/roundup-qa-output.tgz -C /tmp roundup-qa-output' EXIT
git -c credential.helper= clone --no-checkout --single-branch --branch main https://github.com/${repo}.git /tmp/roundup-qa
cd /tmp/roundup-qa
git -c credential.helper= fetch origin ${head}
git checkout --detach ${head}
git update-ref refs/remotes/origin/main ${head}
test "$(git rev-parse HEAD)" = ${head}
pnpm install --frozen-lockfile
QA_SWEEP_PORT=5199 QA_SWEEP_OUT=/tmp/roundup-qa-output bash loop/qa-sweep.sh
find /tmp/roundup-qa-output -name 'sweep-*.json' | grep -q .`;

    hasFailed(await command(["boxd", "machine", "exec", vm, "--timeout", "420", "--", script]));
    await copyOutput(state, edge, output);
  } catch (error) {
    state.failure = `QA boundary failed: ${String(error)}. ${action}`;
  } finally {
    if (record.cleanup === "pending") {
      const removed = await edge.command(["boxd", "machine", "remove", vm, "--confirm", "--json"], 30_000, `${output}/cleanup.log`);
      record = { ...record, cleanup: removed.kind === "ok" ? "removed" : "unknown" };
      const error = detail(removed);

      if (error) state.failure = `${state.failure ?? ""} Cleanup ${error}. ${action}`;
    }

    record = { ...record, ended: new Date(edge.now()).toISOString(), status: state.failure ? "failed" : "passed" };
    state.latest = record;

    if (record.cleanup !== "unknown") state.lease = null;
    edge.record(state);
  }
}
