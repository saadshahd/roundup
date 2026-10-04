import { expect, test } from "bun:test";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { coordinate, type State } from "./qa-coordinator";
import { command, check } from "./qa-coordinator.run";

test("l66_bun_spawn_captures_failed_cli_output", async () => {
  const dir = mkdtempSync(join(tmpdir(), "l66-"));

  try {
    const output = join(dir, "command.log");
    const result = await command(["bash", "-c", "echo fake-cli-output; echo fake-cli-error >&2; exit 7"], 1000, output);
    expect(result).toMatchObject({ kind: "failed" });
    expect(readFileSync(output, "utf8")).toContain("fake-cli-output");
    expect(readFileSync(output, "utf8")).toContain("fake-cli-error");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("l66_bun_spawn_bounds_hanging_cli_as_unknown", async () => {
  const dir = mkdtempSync(join(tmpdir(), "l66-"));

  try {
    const result = await command(["bash", "-c", "sleep 30"], 20, join(dir, "command.log"));
    expect(result.kind).toBe("unknown");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

for (const restarted of [false, true]) {
  test(`l66_fake_cli_${restarted ? "stale_restart" : "sweep"}_copies_evidence_before_removal`, async () => {
    const dir = mkdtempSync(join(tmpdir(), "l66-cli-"));
    const path = Bun.env.PATH;
    const sha = "b".repeat(40);
    const state: State = { due: 0, lease: null, latest: null, failure: null };

    if (restarted) {
      state.lease = -500_001;
      state.latest = { head: sha, started: "2026-10-03T11:00:00Z", ended: null, status: "running", output: join(dir, "original"), cleanup: "pending" };
    }

    try {
      writeFileSync(join(dir, "curl"), `#!/bin/sh
printf '%s' '${sha}'
`, { mode: 0o755 });
      writeFileSync(join(dir, "boxd"), `#!/bin/sh
set -eu
case "$1 $2" in
  'machine list') printf '[]' ;;
  'machine exec') test "$4" = --timeout; test "$5" = 420; test "$6" = --; printf '%s' "$7" | grep -q 'git checkout --detach ${sha}' ;;
  'machine cp') test "$3" = ru-qa-scheduled:/tmp/roundup-qa-output.tgz; printf 'fake archive' > "$4"; touch "$0.copied" ;;
  'machine remove') test "$3" = ru-qa-scheduled; test "$4" = --confirm; test -f "$0.copied" ;;
  *) exit 99 ;;
esac
`, { mode: 0o755 });
      writeFileSync(join(dir, "bun"), `#!/bin/sh
test "$2" = create
`, { mode: 0o755 });
      Bun.env.PATH = `${dir}:${path}`;
      await coordinate(state, { now: () => 0, command, record: () => {} }, "example/roundup", dir);

      if (restarted) expect(state.failure).toContain("stale lease");
      else expect(state.failure).toBeNull();
      expect(state.latest).toMatchObject({ head: sha, status: restarted ? "failed" : "passed", cleanup: "removed" });
      expect(readFileSync(`${state.latest?.output}/sweep.tgz`, "utf8")).toBe("fake archive");
    } finally {
      Bun.env.PATH = path;
      rmSync(dir, { recursive: true, force: true });
    }
  });
}

for (const [name, due, lease, failure, status, now, kind] of [
  ["healthy", 600_000, null, null, "passed", 0, "ok"],
  ["missed", 0, null, null, "passed", 60_001, "failed"],
  ["stale", 600_000, 0, null, "running", 500_001, "failed"],
  ["latched", 600_000, null, "sweep failed", "passed", 0, "failed"],
  ["malformed", "invalid", null, null, "passed", 0, "failed"],
  ["empty", 600_000, null, null, null, 0, "failed"],
] as const) {
  test(`l66_disk_check_${name}`, async () => {
    const dir = mkdtempSync(join(tmpdir(), "l66-check-"));

    try {
      writeFileSync(join(dir, "state.json"), JSON.stringify({ due, lease, failure, latest: { status } }));
      expect((await check(dir, now)).kind).toBe(kind);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
}

for (const [name, script] of [
  ["rejected", "process.exitCode = 1;"],
  ["hanging", "await Bun.sleep(30_000);"],
] as const) {
  test(`l66_sdk_child_${name}_is_unknown`, async () => {
    const dir = mkdtempSync(join(tmpdir(), "l66-sdk-"));

    try {
      const file = join(dir, "create.ts");
      writeFileSync(file, script);
      const result = await command(["bun", file, "create"], name === "rejected" ? 5000 : 100, join(dir, "commands.log"));
      expect(result.kind).toBe("unknown");
      expect(result).toHaveProperty("detail", expect.stringContaining(name === "rejected" ? "exit 1" : "exit 124"));
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
}
