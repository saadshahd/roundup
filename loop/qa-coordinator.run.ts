import { appendFileSync, closeSync, existsSync, mkdirSync, openSync, readdirSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { coordinate, inspect, interval, type CommandResult, type State } from "./qa-coordinator";

const out = resolve(import.meta.dir, "out/qa-scheduled");

const lock = `${out}/lease`;

export async function command(args: string[], timeoutMs: number, output: string): Promise<CommandResult> {
  let fd: number | null = null;

  try {
    mkdirSync(dirname(output), { recursive: true });
    fd = openSync(output, output.endsWith("/commands.log") ? "a" : "w");
    const capture = args[0] === "curl" || args[0] === "bash" || args[0] === "jq";

    const child = Bun.spawn(["timeout", "--signal=TERM", "--kill-after=2s", `${Math.max(1, timeoutMs) / 1000}s`, ...args], {
      env: { ...Bun.env }, stdin: "ignore", stdout: capture ? "pipe" : fd, stderr: fd,
    });

    const text = capture ? await new Response(child.stdout).text() : "";
    const code = await child.exited;

    if (capture) appendFileSync(output, text);

    if (code === 0) return { kind: "ok", output: text };

    return { kind: code === 124 || code === 137 ? "unknown" : "failed", detail: `${args.slice(0, 3).join(" ")} exit ${code}; see ${output}` };
  } catch (error) {
    return { kind: "unknown", detail: `${args.slice(0, 3).join(" ")}: ${String(error)}; see ${output}` };
  } finally {
    if (fd !== null) closeSync(fd);
  }
}

function record(state: State): void {
  const line = JSON.stringify(state);
  mkdirSync(out, { recursive: true });
  writeFileSync(`${out}/state.tmp`, `${line}\n`);
  renameSync(`${out}/state.tmp`, `${out}/state.json`);
  console.log(line);
  const retained = readdirSync(out).filter(name => /^\d{4}-\d{2}-\d{2}T/.test(name)).sort();

  for (const name of retained.slice(0, -144)) rmSync(`${out}/${name}`, { recursive: true, force: true });
}

export async function tick(state: State, edge: Parameters<typeof coordinate>[1], repo: string, out: string, maxVMs: number): Promise<void> {
  const lock = `${out}/lease`;
  mkdirSync(out, { recursive: true });

  if (existsSync(lock)) {
    if (state.lease !== null && inspect(state, edge.now()) === null) return;
    state.failure = inspect(state, edge.now()) ?? "QA lease directory remains: stop the job and follow docs/qa-coordinator.md recovery";
    await coordinate(state, edge, repo, out, maxVMs);

    return;
  }

  // mkdir is atomic across replacement jobs; a crashed holder must be acknowledged, never stolen.
  try {
    mkdirSync(lock);
  } catch (error) {
    state.failure = `Cannot acquire QA lease: ${String(error)}; stop the job and follow docs/qa-coordinator.md`;
    edge.record(state);

    return;
  }

  try {
    await coordinate(state, edge, repo, out, maxVMs);
  } finally {
    rmSync(lock, { recursive: true, force: true });
  }
}

export async function check(out: string, now: number): Promise<CommandResult> {
  const result = await command(["jq", "-ce", `
    {due, lease, failure, status: .latest.status} |
    select((.due | type == "number") and
      (.lease == null or (.lease | type == "number")) and
      (.failure == null or (.failure | type == "string")) and
      (.status as $status | [null, "running", "passed", "failed"] | index($status) != null))`, `${out}/state.json`], 5000, `${out}/check.log`);

  if (result.kind !== "ok") return result;
  // SAFETY: jq establishes these field types before JSON reaches the shared health rules.
  const state = JSON.parse(result.output) as Pick<State, "due" | "lease" | "failure"> & { status: "running" | "passed" | "failed" | null };
  const problem = inspect(state, now) ?? (state.status === null || state.status === "failed" ? "No successful QA result; inspect Run logs and docs/qa-coordinator.md" : null);

  return problem ? { kind: "failed", detail: problem } : result;
}

async function main(): Promise<void> {
  const mode = Bun.argv[2] ?? Bun.env.ROUNDUP_QA_MODE ?? "register";

  if (mode === "dry-run") {
    const child = Bun.spawn(["bun", "test", `${import.meta.dir}/qa-coordinator.test.ts`, `${import.meta.dir}/qa-command.test.ts`], { stdout: "inherit", stderr: "inherit" });
    process.exitCode = await child.exited;

    return;
  }

  if (mode === "check") {
    const result = await check(out, Date.now());
    console.log(result.kind === "ok" ? result.output : result.detail);
    process.exitCode = result.kind === "ok" ? 0 : 1;

    return;
  }

  if (mode !== "register" && mode !== "recover") throw new Error("Use register, dry-run, check or recover");
  const repo = Bun.env.ROUNDUP_QA_REPO ?? "";

  if (!/^[A-Za-z0-9_.-]+\/roundup$/.test(repo)) throw new Error("Set ROUNDUP_QA_REPO to the trusted public owner/roundup before registration");
  const maxVMs = Number(Bun.env.BOXD_MAX_VMS ?? "12");

  if (!Number.isSafeInteger(maxVMs) || maxVMs < 1) throw new Error("BOXD_MAX_VMS must be a positive integer");
  const { every, object } = await import("@boxd/run");
  const state = object<State>("roundup-qa-scheduled");
  state.due ??= Date.now() + interval;
  state.lease ??= null;
  state.latest ??= null;
  state.failure ??= null;

  if (mode === "recover") {
    const absent = await command(["bash", "-o", "pipefail", "-c", 'boxd machine list --json | jq -e \'all(.[]; .name != "ru-qa-scheduled")\''], 30_000, `${out}/recover.log`);

    if (absent.kind !== "ok") throw new Error(`Recovery refused: ${absent.detail}`);
    console.log(`Acknowledged: ${JSON.stringify(state)}`);
    state.failure = null;
    state.lease = null;
    state.due = Date.now() + interval;
    rmSync(lock, { recursive: true, force: true });
    record(state);

    return;
  }

  const edge = { now: Date.now, command, record };
  every("10 minutes", () => tick(state, edge, repo, out, maxVMs));
  record(state);
}

if (import.meta.main) await main();
