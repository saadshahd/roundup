import { expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { create, tick } from "./qa-coordinator.run";
import { coordinate, inspect, type State, type CommandResult } from "./qa-coordinator";

const sha = "a".repeat(40);

function fixture() {
  let now = Date.parse("2026-10-03T12:00:00Z");
  const state: State = { due: now, lease: null, latest: null, failure: null };
  const calls: string[][] = [];
  const logs: string[] = [];
  let fail = "";

  const edge = {
    now: () => now,
    command: async (args: string[], _timeout: number, _output: string): Promise<CommandResult> => {
      calls.push(args);

      if (args.join(" ").includes(fail) && fail) return { kind: "failed", detail: "fake failure" };

      if (args[0] === "curl") return { kind: "ok", output: sha };

      if (args.join(" ").includes("machine list")) return { kind: "ok", output: "0" };

      return { kind: "ok", output: "" };
    },
    record: (value: State) => { logs.push(JSON.stringify(value)); },
  };

  return { state, calls, logs, edge, advance: (ms: number) => { now += ms; }, fail: (text: string) => { fail = text; } };
}

const repo = "example/roundup";

test("l66_success_records_exact_main_and_removes_vm", async () => {
  const f = fixture();
  await coordinate(f.state, f.edge, repo, "/proof");
  expect(f.state.latest).toMatchObject({ head: sha, started: "2026-10-03T12:00:00.000Z", ended: "2026-10-03T12:00:00.000Z", status: "passed", cleanup: "removed" });
  expect(f.state.failure).toBeNull();
  expect(f.calls.find(c => c.includes("create"))).toEqual(["bun", `${import.meta.dir}/qa-coordinator.run.ts`, "create"]);
  const script = f.calls.find(c => c.includes("exec"))?.at(-1) ?? "";
  expect(f.calls.find(c => c.includes("exec"))?.slice(0, 6)).toEqual(["boxd", "machine", "exec", "ru-qa-scheduled", "--timeout", "420"]);
  expect(script).toContain(sha);
  expect(script).toContain("loop/qa-sweep.sh");
  expect(script).toContain("QA_SWEEP_PORT=5199");
  expect(script).not.toContain("QA_SWEEP_TEST_MODE");
  expect(f.calls.at(-1)).toEqual(["boxd", "machine", "remove", "ru-qa-scheduled", "--confirm", "--json"]);
});

for (const phase of ["curl", "create", "machine exec", "machine cp", "machine remove"]) {
  test(`l66_${phase.replaceAll(" ", "_")}_failure_stays_red`, async () => {
    const f = fixture();
    f.fail(phase);
    await coordinate(f.state, f.edge, repo, "/proof");
    expect(f.state.failure).toContain("fake failure");
    expect(f.state.latest?.status).toBe("failed");
    const count = f.calls.length;
    f.advance(600_000);
    await coordinate(f.state, f.edge, repo, "/proof");
    expect(f.calls.length).toBe(count + (phase === "machine remove" ? 3 : 0));
    expect(f.logs.at(-1)).toContain("fake failure");

    if (phase !== "curl") expect(f.calls.at(-1)?.[2]).toBe("remove");
  });
}

test("l66_overlapping_tick_does_not_create_a_second_vm", async () => {
  const f = fixture();
  const first = coordinate(f.state, f.edge, repo, "/proof");
  await coordinate(f.state, f.edge, repo, "/proof");
  await first;
  expect(f.calls.filter(c => c.includes("create"))).toHaveLength(1);
});

test("l66_stale_lease_after_restart_removes_without_provisioning", async () => {
  const f = fixture();
  f.state.lease = f.edge.now() - 500_001;
  await coordinate(f.state, f.edge, repo, "/proof");
  expect(f.state.failure).toContain("stale lease");
  expect(f.calls.map(c => c[2])).toEqual(["cp", "/proof/sweep.tgz", "remove"]);
});

test("l66_missed_tick_is_red_without_a_wake", () => {
  const f = fixture();
  f.advance(60_001);
  expect(inspect(f.state, f.edge.now())).toContain("missed");
});

test("l66_late_wake_persists_failure_without_starting_vm", async () => {
  const f = fixture();
  f.advance(60_001);
  await coordinate(f.state, f.edge, repo, "/proof");
  expect(f.state.failure).toContain("missed");
  expect(f.calls).toHaveLength(0);
});

test("l66_timeout_is_unknown_and_still_removes", async () => {
  const f = fixture();
  const command = f.edge.command;
  f.edge.command = async (args, timeout, output) => args.includes("exec")
    ? { kind: "unknown", detail: "timed out" }
    : command(args, timeout, output);
  await coordinate(f.state, f.edge, repo, "/proof");
  expect(f.state.failure).toContain("unknown: timed out");
  expect(f.calls.at(-1)?.[2]).toBe("remove");
});

test("l66_invalid_api_sha_never_provisions", async () => {
  const f = fixture();
  f.edge.command = async () => ({ kind: "ok", output: '{"message":"rate limited"}' });
  await coordinate(f.state, f.edge, repo, "/proof");
  expect(f.state.failure).toContain("no exact main SHA");
  expect(f.state.latest?.head).toBeNull();
});

test("l66_full_vm_capacity_does_not_create_or_remove", async () => {
  const f = fixture();
  const command = f.edge.command;
  f.edge.command = async (args, timeout, output) => args.join(" ").includes("machine list")
    ? { kind: "ok", output: "12" }
    : command(args, timeout, output);
  await coordinate(f.state, f.edge, repo, "/proof");
  expect(f.state.failure).toContain("VM limit");
  expect(f.calls.some(c => c.includes("create") || c.includes("remove"))).toBe(false);
});

test("l66_global_deadline_stops_work_but_allows_cleanup", async () => {
  const f = fixture();
  const command = f.edge.command;
  f.edge.command = async (args, timeout, output) => {
    const result = await command(args, timeout, output);

    if (args.includes("create")) f.advance(420_001);

    return result;
  };

  await coordinate(f.state, f.edge, repo, "/proof");
  expect(f.state.failure).toContain("deadline exceeded");
  expect(f.calls.some(c => c.includes("exec"))).toBe(false);
  expect(f.state.latest?.cleanup).toBe("removed");
});

test("l66_uncertain_creation_is_removed_without_exec", async () => {
  const f = fixture();
  const command = f.edge.command;
  f.edge.command = async (args, timeout, output) => args.includes("create")
    ? { kind: "unknown", detail: "creation reply lost" }
    : command(args, timeout, output);
  await coordinate(f.state, f.edge, repo, "/proof");
  expect(f.state.failure).toContain("unknown: creation reply lost");
  expect(f.calls.some(c => c.includes("exec"))).toBe(false);
  expect(f.state.latest?.cleanup).toBe("removed");
});

test("l66_empty_copied_archive_is_red", async () => {
  const f = fixture();
  const command = f.edge.command;
  f.edge.command = async (args, timeout, output) => args[0] === "test"
    ? { kind: "failed", detail: "archive absent" }
    : command(args, timeout, output);
  await coordinate(f.state, f.edge, repo, "/proof");
  expect(f.state.failure).toContain("Missing sweep archive");
  expect(f.state.latest?.cleanup).toBe("removed");
});

test("l66_directory_lease_skips_live_overlap_without_marking_red", async () => {
  const f = fixture();
  const out = mkdtempSync(join(tmpdir(), "l66-lease-"));

  try {
    const first = tick(f.state, f.edge, repo, out, 12);
    await tick(f.state, f.edge, repo, out, 12);
    await first;
    expect(f.state.failure).toBeNull();
    expect(f.calls.filter(c => c.includes("create"))).toHaveLength(1);
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
});

for (const outcome of ["ok", "failed", "unknown"] as const) {
  test(`l66_stale_restart_copies_${outcome}_before_removal`, async () => {
    const f = fixture();
    f.state.lease = f.edge.now() - 500_001;
    f.state.latest = { head: sha, started: "2026-10-03T11:00:00Z", ended: null, status: "running", output: "/proof/original", cleanup: "pending" };
    const command = f.edge.command;
    f.edge.command = async (args, timeout, output) => {
      if (args.includes("cp")) {
        expect(args.at(-1)).toBe("/proof/original/sweep.tgz");
        expect(timeout).toBe(30_000);
        expect(output).toBe("/proof/original/copy.log");
        f.calls.push(args);

        return outcome === "ok" ? { kind: "ok", output: "" } : { kind: outcome, detail: "copy unavailable" };
      }

      return command(args, timeout, output);
    };

    await coordinate(f.state, f.edge, repo, "/proof");
    expect(f.calls[0]?.[2]).toBe("cp");
    expect(f.calls.at(-1)?.[2]).toBe("remove");
    expect(f.calls.some(c => c.includes("create"))).toBe(false);
    expect(f.state.latest).toMatchObject({ head: sha, output: "/proof/original", status: "failed", cleanup: "removed" });
    expect(f.state.lease).toBeNull();
    expect(f.state.failure).toContain("stale lease");

    if (outcome !== "ok") expect(f.state.failure).toContain("copy unavailable");
  });
}

test("l66_check_includes_both_coordinator_suites", async () => {
  const child = Bun.spawn(["just", "--dry-run", "check"], { stdout: "pipe", stderr: "pipe" });
  const output = await new Response(child.stderr).text();
  expect(await child.exited).toBe(0);
  expect(output).toContain("bun test loop/qa-coordinator.test.ts loop/qa-command.test.ts");
});

test("l66_sdk_creation_preserves_snapshot_isolation_and_timers", async () => {
  const calls: unknown[] = [];
  await create({ create: async params => { calls.push(params); } });
  expect(calls).toEqual([{
    name: "ru-qa-scheduled",
    fromSnapshot: "ru-toolchain",
    isolated: true,
    config: { autoDestroyTimeout: 540, autoSuspendTimeout: 0 },
  }]);
});


test("l66_sdk_creation_uses_remaining_deadline_after_pending_cleanup_is_recorded", async () => {
  const f = fixture();
  const command = f.edge.command;
  f.edge.command = async (args, timeout, output) => {
    if (args[0] === "curl") f.advance(1234);

    if (args.includes("create")) {
      expect(timeout).toBe(420_000 - 1234);
      expect(JSON.parse(f.logs.at(-1) ?? "{}").latest.cleanup).toBe("pending");
    }

    return command(args, timeout, output);
  };

  await coordinate(f.state, f.edge, repo, "/proof");
  expect(f.state.latest?.status).toBe("passed");
});
