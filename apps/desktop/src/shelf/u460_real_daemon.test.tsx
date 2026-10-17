// @vitest-environment jsdom
import { connect } from "node:net";
import * as v from "valibot";
import { readdirSync, existsSync } from "node:fs";
import { join } from "node:path";
import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterAll, afterEach, beforeAll, expect, it } from "vitest";
import { App } from "../App";
import { fakeEmulators } from "../terminal/paneHarness";
import { createServedApp } from "../testing/realSeam";
import { startRealDaemon } from "../testing/realDaemonFixture";
import type { StartedDaemon } from "../testing/realDaemonFixture";
import { showEverythingInApp } from "../testing/showEverything";
import { showShelfTab } from "../testing/shelfTabs";
import { askToDelete, confirmDeletion, stubDialogLayout } from "./bulkQuestionHarness";

let real: StartedDaemon;

const replySchema = v.object({ id: v.number(), result: v.optional(v.unknown()), error: v.optional(v.object({ code: v.number(), message: v.string() })) });

const ownerCall = (owner: string, method: "pad.create" | "pad.write" | "pad.append", name: string, text: string): Promise<v.InferOutput<typeof replySchema>> => new Promise((resolve, reject) => {
  const socket = connect(join(real.dir, "rupd.sock"));
  const timer = setTimeout(() => { socket.destroy(); reject(new Error("owner call timed out")); }, 20_000);
  let buffer = "";
  socket.setEncoding("utf8");
  socket.on("connect", () => socket.write(JSON.stringify({ jsonrpc: "2.0", id: 1, method: "daemon.identify", params: { actor: { kind: "agent", id: owner, parent: null } } }) + "\n"));
  socket.on("data", (chunk: string) => {
    buffer += chunk;

    for (;;) {
      const end = buffer.indexOf("\n");

      if (end < 0) break;
      const frame = v.parse(replySchema, JSON.parse(buffer.slice(0, end)));
      buffer = buffer.slice(end + 1);

      if (frame.id === 1 && !frame.error) socket.write(JSON.stringify({ jsonrpc: "2.0", id: 2, method, params: { name, text } }) + "\n");
      else {
        clearTimeout(timer);
        socket.destroy();
        resolve(frame);
      }
    }
  });
  socket.on("error", (error) => { clearTimeout(timer); socket.destroy(); reject(error); });
});

beforeAll(async () => {
  real = await startRealDaemon({});
}, 120_000);

afterAll(async () => { await real?.stop(); });

afterEach(cleanup);

stubDialogLayout();

it("u460_bulk_todo_delete_preserves_t6_across_homes", async () => {
  const app = createServedApp(real.base);
  const one = await app.rpc("rail.createWorkstream", { name: "bulk-scope", parent: null });
  const other = await app.rpc("rail.createWorkstream", { name: "elsewhere", parent: null });
  const first = await app.rpc("todo.create", { title: "blocker one", body: null, blockers: null });
  const second = await app.rpc("todo.create", { title: "blocker two", body: null, blockers: null });
  await app.rpc("todo.move", { id: first.id, home: one.id });
  await app.rpc("todo.move", { id: second.id, home: one.id });
  const dependent = await app.rpc("todo.create", { title: "dependent", body: null, blockers: [first.id, second.id] });
  await app.rpc("todo.move", { id: dependent.id, home: other.id });
  render(() => <App app={app} reducedMotion={() => true} clock={Date.now} createEmulator={fakeEmulators().factory} />);
  fireEvent.click(await screen.findByRole("treeitem", { name: /bulk-scope/ }));
  await showShelfTab("Todos");
  fireEvent.click(await screen.findByRole("button", { name: new RegExp(`#${first.id} `) }), { metaKey: true });
  fireEvent.click(screen.getByRole("button", { name: new RegExp(`#${second.id} `) }), { metaKey: true });
  await askToDelete(2);
  await confirmDeletion();
  await waitFor(async () => {
    const rows = await app.rpc("todo.list", null);
    expect(rows.find((row) => row.id === dependent.id)?.blockers).toEqual([]);
    expect(rows.some((row) => row.id === first.id || row.id === second.id)).toBe(false);
  });
}, 60_000);

for (const files of [false, true]) it(`u460_deleted_pad_updates_do_not_recreate_it files=${files}`, async () => {
  const app = createServedApp(real.base);
  await app.rpc("pad.setStorage", { files });
  const name = `bulk-pad-${files}`;
  const owner = (await app.rpc("rail.tree", null)).find((row) => row.name === "bulk-scope")!;
  expect((await ownerCall(owner.id, "pad.create", name, "before")).error).toBeUndefined();
  expect((await ownerCall(owner.id, "pad.write", name, "committed before delete")).error).toBeUndefined();
  render(() => <App app={app} reducedMotion={() => true} clock={Date.now} createEmulator={fakeEmulators().factory} />);
  await showEverythingInApp(/bulk-scope/);
  await showShelfTab("Pads");
  fireEvent.click(await screen.findByRole("button", { name }), { metaKey: true });
  await askToDelete(1);
  await confirmDeletion();
  await waitFor(async () => expect((await app.rpc("pad.list", null)).some((row) => row.name === name)).toBe(false));
  expect((await ownerCall(owner.id, "pad.write", name, "after")).error?.code).toBe(-32001);
  expect((await ownerCall(owner.id, "pad.append", name, "after")).error?.code).toBe(-32001);
  await expect(app.rpc("pad.read", { name })).rejects.toMatchObject({ code: -32001 });
  const projects = join(real.dataDir, "projects");

  const keys = readdirSync(projects);

  expect(keys.length).toBeGreaterThan(0);

  for (const key of keys) expect(existsSync(join(projects, key, "pads", `${name}.md`))).toBe(false);
  expect((await app.rpc("pad.create", { name, text: "explicit new Pad" })).text).toBe("explicit new Pad");
  await app.rpc("pad.delete", { name });
}, 60_000);

it("u460_bulk_todo_delete_preserves_t6_across_homes_with_a_failed_blocker", async () => {
  const app = createServedApp(real.base);
  const homes = await app.rpc("rail.tree", null);
  const one = homes.find((row) => row.name === "bulk-scope")!;
  const other = homes.find((row) => row.name === "elsewhere")!;
  const first = await app.rpc("todo.create", { title: "partial one", body: null, blockers: null });
  const second = await app.rpc("todo.create", { title: "partial two", body: null, blockers: null });
  await app.rpc("todo.move", { id: first.id, home: one.id });
  await app.rpc("todo.move", { id: second.id, home: one.id });
  const dependent = await app.rpc("todo.create", { title: "partial dependent", body: null, blockers: [first.id, second.id] });
  await app.rpc("todo.move", { id: dependent.id, home: other.id });

  const failedApp: typeof app = {
    ...app,
    rpc: (method, params) => method === "todo.delete" && params && "id" in params && params.id === second.id ? Promise.reject(new Error("injected unavailable connection")) : app.rpc(method, params),
    forProject: () => failedApp,
  };

  render(() => <App app={failedApp} reducedMotion={() => true} clock={Date.now} createEmulator={fakeEmulators().factory} />);
  fireEvent.click(await screen.findByRole("treeitem", { name: /bulk-scope/ }));
  await showShelfTab("Todos");
  fireEvent.click(await screen.findByRole("button", { name: new RegExp(`#${first.id} `) }), { metaKey: true });
  fireEvent.click(screen.getByRole("button", { name: new RegExp(`#${second.id} `) }), { metaKey: true });
  await askToDelete(2);
  await confirmDeletion();
  await screen.findByText(/partial two .*injected unavailable connection/);
  const rows = await app.rpc("todo.list", null);
  expect(rows.find((row) => row.id === dependent.id)).toMatchObject({ blockers: [second.id], blocked: true, home: other.id });
  expect(rows.some((row) => row.id === first.id)).toBe(false);
  expect(rows.some((row) => row.id === second.id)).toBe(true);
}, 60_000);

it("u460_deleted_rows_stay_absent_after_reopening_the_project", async () => {
  await real.restart();
  const app = createServedApp(real.base);
  const todos = await app.rpc("todo.list", null);
  expect(todos.map((todo) => todo.title)).toEqual(["dependent", "partial two", "partial dependent"]);
  expect((await app.rpc("pad.list", null)).some((pad) => pad.name.startsWith("bulk-pad-"))).toBe(false);
  render(() => <App app={app} reducedMotion={() => true} clock={Date.now} createEmulator={fakeEmulators().factory} />);
  fireEvent.click(await screen.findByRole("treeitem", { name: /bulk-scope/ }));
  await showShelfTab("Todos");
  const row = await screen.findByRole("button", { name: /partial two/ });
  expect(row.getAttribute("aria-pressed")).toBe("false");
  expect(screen.queryByRole("group", { name: "Work items deletion" })).toBeNull();
}, 60_000);
