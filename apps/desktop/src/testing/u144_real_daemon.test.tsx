// @vitest-environment jsdom
import { spawn, spawnSync } from "node:child_process";
import type { ChildProcess } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { createServer } from "vite";
import type { ViteDevServer } from "vite";
import { App } from "../App";
import { RpcError } from "../app/seam";
import type { AppSeam } from "../app/seam";
import { fakeEmulators } from "../terminal/paneHarness";
import { createServedApp } from "./realSeam";
import { realDaemon } from "./realDaemonPlugin";

const REPO = resolve(process.cwd(), "../..");

const PROOF = "0".repeat(64);

let dir: string;

let daemon: ChildProcess;

let server: ViteDevServer;

let base: string;

const served = async () => {
  for (let tries = 0; tries < 300; tries += 1) {
    const response = await fetch(`${base}/__daemon/open`).catch(() => null);

    if (response?.ok) return;
    await new Promise((done) => setTimeout(done, 100));
  }

  throw new Error("the dev server never served the Daemon routes");
};

beforeAll(async () => {
  const built = spawnSync("cargo", ["build", "-p", "rupd", "-p", "rup"], { cwd: REPO, stdio: "inherit" });

  if (built.status !== 0) throw new Error("cargo build -p rupd -p rup failed");
  dir = await mkdtemp(join(tmpdir(), "roundup-u144-"));
  const socket = join(dir, "rupd.sock");

  daemon = spawn(join(REPO, "target/debug/rupd"), [dir, "--attached"], {
    env: {
      ...process.env,
      RUPD_SOCKET: socket,
      CLAUDE_CONFIG_DIR: join(dir, "claude-config"),
      ROUNDUP_RUP_BIN: join(REPO, "target/debug/rup"),
      ROUNDUP_CLAUDE_BIN: join(REPO, "crates/rup/tests/e2e/fake_claude.py"),
    },
    stdio: ["pipe", "pipe", "pipe"],
  });
  daemon.stdin!.write(`roundup-proof 1 ${PROOF}\n`);
  await new Promise<void>((serving, fail) => {
    daemon.stderr!.on("data", (chunk: Buffer) => chunk.toString().includes("serving") && serving());
    daemon.on("exit", () => fail(new Error("rupd exited before serving")));
  });
  server = await createServer({
    configFile: join(process.cwd(), "vite.config.ts"),
    root: process.cwd(),
    logLevel: "silent",
    plugins: [realDaemon({ project: dir, socket, proof: PROOF })],
    server: { host: "127.0.0.1", port: 0 },
    optimizeDeps: { noDiscovery: true, include: [] },
  });
  await server.listen();
  base = server.resolvedUrls!.local[0]!.replace(/\/$/, "");
  await served();
}, 120_000);

afterAll(async () => {
  daemon?.kill();
  await server?.close();
  await rm(dir, { recursive: true, force: true });
});

afterEach(cleanup);

const mount = async () => {
  const app: AppSeam = createServedApp(base);
  const emulators = fakeEmulators();

  render(() => <App app={app} reducedMotion={() => false} clock={Date.now} createEmulator={emulators.factory} />);

  return { app, emulators };
};

describe("u144 the harness serves the App on a real Daemon", () => {
  it("u144_a_room_made_and_started_by_pointer_comes_from_the_daemon", async () => {
    const { app, emulators } = await mount();

    fireEvent.click(await screen.findByRole("button", { name: "start a Room" }));

    const rail = await screen.findByRole("region", { name: "rail" });

    await waitFor(() => expect(within(rail).getAllByText("room").length).toBeGreaterThan(0));
    await waitFor(() => expect(emulators.made.size).toBe(1), { timeout: 20_000 });

    const tree = JSON.stringify(await app.rpc("rail.tree", null));
    const terminals = await app.rpc("terminal.list", null);

    expect([tree.includes('"room"'), terminals.length]).toEqual([true, 1]);
    expect([...emulators.made.keys()]).toEqual(terminals.map((terminal) => terminal.id));
  }, 60_000);

  it("u144_the_choosers_answer_null_and_the_dock_badge_does_nothing", async () => {
    const app = createServedApp(base);

    expect([await app.chooseProjectPath(), await app.chooseSavePath("x.md"), await app.setDockBadge(3), (await app.project())?.path, await app.daemonProof()]).toEqual([null, null, undefined, dir, PROOF]);
  });

  it("u144_a_method_the_daemon_rejects_reaches_the_caller_with_its_message", async () => {
    const app = createServedApp(base);

    const failure = await app.rpc("rail.startDoor", { id: "nowhere" }).then(
      () => null,
      (error: RpcError) => error,
    );

    expect([failure instanceof RpcError, failure?.message]).toEqual([true, expect.stringContaining("nowhere")]);
  });

  it("u144_a_method_the_daemon_rejects_shows_its_message_in_the_mounted_app", async () => {
    const served = createServedApp(base);
    let held = false;
    const app: AppSeam = { ...served, subscribe: (onEvent) => served.subscribe((event) => (held ? undefined : onEvent(event))) };
    const room = await served.rpc("rail.createRoom", { name: "doomed", parent: null });

    render(() => <App app={app} reducedMotion={() => false} clock={Date.now} createEmulator={fakeEmulators().factory} />);

    const rail = await screen.findByRole("region", { name: "rail" });

    fireEvent.dblClick(await within(rail).findByText("doomed"));

    const field = await within(rail).findByRole("textbox");

    held = true;
    await served.rpc("rail.remove", { id: room.id });

    const rejected = await served.rpc("rail.rename", { id: room.id, name: "renamed" }).then(
      () => null,
      (error: RpcError) => error,
    );

    fireEvent.input(field, { target: { value: "renamed" } });
    fireEvent.keyDown(field, { key: "Enter" });

    expect(rejected).toBeInstanceOf(RpcError);
    expect((await within(rail).findByRole("alert")).textContent).toContain(rejected!.message);
  }, 60_000);

  it("u144_closing_the_socket_fires_daemon_exited_and_the_header_says_so", async () => {
    await mount();
    await screen.findByRole("banner");

    daemon.stdin!.end();
    daemon.kill();

    expect(await screen.findByText("daemon exited by signal", {}, { timeout: 20_000 })).toBeTruthy();
  }, 60_000);
});
