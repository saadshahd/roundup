import { Channel } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, expectTypeOf, it } from "vitest";
import type { Event as DaemonEvent } from "@contracts/Event";
import type { RailNode } from "@contracts/agent/RailNode";
import type { Todo } from "@contracts/todo/Todo";
import { createFakeApp } from "../testing/fakeApp";
import { connectEvents } from "./events";
import { RpcError } from "./seam";
import { createTauriApp } from "./tauri";

const TODO: Todo = { id: 1, title: "t", body: "", done: false, blockers: [], blocked: false, created_at: 0 };

const NOT_FOUND = -32001;

const created = (id: number): DaemonEvent => ({
  actor: { kind: "user", id: "you", parent: null },
  name: "todo.created",
  data: { ...TODO, id },
});

const calls: unknown[] = [];

afterEach(() => {
  clearMocks();
  calls.length = 0;
});

describe("u1 calls and events", () => {
  it("u1_a_call_is_typed_from_the_method_table", async () => {
    const app = createFakeApp();
    app.handlers["todo.get"] = () => TODO;
    app.handlers["rail.tree"] = () => [];

    expectTypeOf(await app.rpc("todo.get", { id: 1 })).toEqualTypeOf<Todo>();
    expectTypeOf(await app.rpc("rail.tree", null)).toEqualTypeOf<RailNode[]>();
    // @ts-expect-error `todo.get` takes `{ id: number }`; a string id must not compile.
    await app.rpc("todo.get", { id: "1" });
  });

  it("u1_a_call_passes_its_method_and_params_and_resolves_the_result", async () => {
    mockIPC((command, args) => (command === "rpc" ? { echoed: args } : null));

    const result = await createTauriApp().rpc("todo.get", { id: 7 });

    expect(result).toEqual({ echoed: { method: "todo.get", params: { id: 7 } } });
  });

  it("u1_a_failed_call_rejects_with_the_daemons_code_and_message", async () => {
    mockIPC(() => {
      throw { code: NOT_FOUND, message: "no todo 99" };
    });

    const failure = createTauriApp().rpc("todo.get", { id: 99 });

    await expect(failure).rejects.toBeInstanceOf(RpcError);
    await expect(failure).rejects.toMatchObject({ code: NOT_FOUND, message: "no todo 99" });
  });

  it("u1_a_rejection_that_is_not_a_code_and_message_becomes_internal", async () => {
    mockIPC(() => {
      throw "command rpc not found";
    });

    const failure = createTauriApp().rpc("daemon.ping", null);

    await expect(failure).rejects.toMatchObject({ code: -32603, message: "command rpc not found" });
  });

  it("u1_the_daemons_events_reach_the_subscription_in_order", async () => {
    const received: DaemonEvent[] = [];
    let sent: (event: DaemonEvent) => void = () => {};

    mockIPC((command, args) => {
      if (command === "subscribe" && args && "channel" in args && args.channel instanceof Channel) {
        sent = args.channel.onmessage;
      }

      return null;
    });

    await createTauriApp().subscribe((event) => received.push(event));

    for (const id of [1, 2, 3]) sent(created(id));

    expect(received).toEqual([created(1), created(2), created(3)]);
  });

  it("u1_every_subscriber_gets_every_event_in_the_order_the_daemon_sent_it", async () => {
    const app = createFakeApp();
    const first: DaemonEvent[] = [];
    const second: DaemonEvent[] = [];

    const events = await connectEvents(app);
    events.subscribe((event) => first.push(event));
    events.subscribe((event) => second.push(event));

    for (const id of [1, 2, 3]) app.emit(created(id));

    expect(first).toEqual([created(1), created(2), created(3)]);
    expect(second).toEqual(first);
  });

  it("u1_a_subscriber_that_unsubscribed_hears_nothing_more", async () => {
    const app = createFakeApp();
    const heard: DaemonEvent[] = [];

    const events = await connectEvents(app);
    const unsubscribe = events.subscribe((event) => heard.push(event));
    app.emit(created(1));
    unsubscribe();
    app.emit(created(2));

    expect(heard).toEqual([created(1)]);
  });

  it("u1_the_adapter_listens_for_daemon_exited", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const exits: unknown[] = [];

    await createTauriApp().onDaemonExited((exit) => exits.push(exit));
    await emit("daemon-exited", { code: null });

    expect(exits).toEqual([{ code: null }]);
  });

  it("u1_the_adapter_stops_listening_once_unsubscribed", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const exits: unknown[] = [];

    const unsubscribe = await createTauriApp().onDaemonExited((exit) => exits.push(exit));
    unsubscribe();
    await emit("daemon-exited", { code: 1 });

    expect(exits).toEqual([]);
  });

  it("u1_the_adapter_opens_the_macos_folder_chooser_for_a_directory", async () => {
    mockIPC((command, args) => {
      if (command === "plugin:dialog|open") calls.push(args);

      return "/Users/me/p";
    });

    const path = await createTauriApp().chooseProjectPath();

    expect([path, calls]).toEqual(["/Users/me/p", [{ options: { directory: true, multiple: false } }]]);
  });

  it("u1_the_adapter_opens_the_save_chooser_with_the_suggested_file_name", async () => {
    mockIPC((command, args) => {
      if (command === "plugin:dialog|save") calls.push(args);

      return "/tmp/auth-notes.md";
    });

    const path = await createTauriApp().chooseSavePath("auth-notes.md");

    expect([path, calls]).toEqual(["/tmp/auth-notes.md", [{ options: { defaultPath: "auth-notes.md" } }]]);
  });

  it("u1_a_cancelled_chooser_answers_null", async () => {
    mockIPC(() => null);

    expect(await createTauriApp().chooseSavePath("x.md")).toBeNull();
  });

  it("u1_the_adapter_sets_the_dock_badge_count", async () => {
    mockWindows("main");
    mockIPC((command, args) => {
      if (command === "plugin:window|set_badge_count") calls.push(args);

      return null;
    });

    await createTauriApp().setDockBadge(3);

    expect(calls).toEqual([{ label: "main", value: 3 }]);
  });

  it("u1_a_dock_badge_count_of_zero_clears_the_badge", async () => {
    mockWindows("main");
    mockIPC((command, args) => {
      if (command === "plugin:window|set_badge_count") calls.push(args);

      return null;
    });

    await createTauriApp().setDockBadge(0);

    expect(calls).toEqual([{ label: "main", value: undefined }]);
  });

  it("u1_the_adapter_parses_the_open_project", async () => {
    mockIPC(() => ({ name: "payments-api", path: "/p/payments-api" }));

    expect(await createTauriApp().project()).toEqual({ name: "payments-api", path: "/p/payments-api" });
  });

  it("u1_the_adapter_answers_null_when_no_project_is_open", async () => {
    mockIPC(() => null);

    expect(await createTauriApp().project()).toBeNull();
  });

  it.each([{ name: 7, path: "/p" }, { name: "p", path: 7 }, { name: "p" }, { path: "/p" }])(
    "u1_a_project_answer_of_%j_is_rejected_as_internal_by_project_and_open_project",
    async (answer) => {
      mockIPC(() => answer);
      const app = createTauriApp();

      await expect(app.project()).rejects.toMatchObject({ name: "RpcError", code: -32603 });
      await expect(app.openProject("/p")).rejects.toMatchObject({ name: "RpcError", code: -32603 });
    },
  );

  it("u1_a_daemon_exited_listener_that_cannot_attach_rejects_with_internal_and_the_string", async () => {
    mockIPC((command) => {
      if (command === "plugin:event|listen") throw "no event channel";

      return null;
    });

    await expect(createTauriApp().onDaemonExited(() => {})).rejects.toMatchObject({
      name: "RpcError",
      code: -32603,
      message: "no event channel",
    });
  });

  it("u1_a_daemon_exited_payload_that_is_not_a_code_never_reaches_the_listener", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const exits: unknown[] = [];

    await createTauriApp().onDaemonExited((exit) => exits.push(exit));
    await emit("daemon-exited", { code: "x" }).catch(() => {});

    expect(exits).toEqual([]);
  });

  it("u1_a_folder_chooser_that_throws_a_string_rejects_with_internal_and_the_string", async () => {
    mockIPC((command) => {
      if (command === "plugin:dialog|open") throw "dialog could not open";

      return null;
    });

    await expect(createTauriApp().chooseProjectPath()).rejects.toMatchObject({
      name: "RpcError",
      code: -32603,
      message: "dialog could not open",
    });
  });

  it("u1_a_save_chooser_that_throws_a_string_rejects_with_internal_and_the_string", async () => {
    mockIPC((command) => {
      if (command === "plugin:dialog|save") throw "save panel failed";

      return null;
    });

    await expect(createTauriApp().chooseSavePath("x.md")).rejects.toMatchObject({
      code: -32603,
      message: "save panel failed",
    });
  });

  it("u1_a_dock_badge_that_throws_a_string_rejects_with_internal_and_the_string", async () => {
    mockWindows("main");
    mockIPC((command) => {
      if (command === "plugin:window|set_badge_count") throw "no dock";

      return null;
    });

    await expect(createTauriApp().setDockBadge(2)).rejects.toMatchObject({ code: -32603, message: "no dock" });
  });

  it("u1_a_malformed_project_answer_rejects_with_the_validation_message", async () => {
    mockIPC(() => ({ name: 7, path: "/p" }));

    await expect(createTauriApp().project()).rejects.toMatchObject({
      message: "Invalid type: Expected string but received 7",
    });
  });

  it("u1_the_adapter_file_is_the_only_door_to_tauri", async () => {
    type FsModule = {
      readdirSync: (path: string, options: { withFileTypes: true }) => { name: string; isDirectory(): boolean }[];
      readFileSync: (path: string, encoding: "utf8") => string;
    };

    type PathModule = { dirname: (path: string) => string; join: (...paths: string[]) => string };

    type UrlModule = { fileURLToPath: (url: string) => string };

    // SAFETY: this app has no @types/node; specifier is non-literal so tsc cannot resolve the real "node:*" shape, and the three node: built-ins loaded below are typed by hand instead.
    const nodeModule = <T>(specifier: string): Promise<T> => import(specifier) as Promise<T>;

    const fs = await nodeModule<FsModule>("node:fs");
    const path = await nodeModule<PathModule>("node:path");
    const url = await nodeModule<UrlModule>("node:url");

    const thisFile = url.fileURLToPath(import.meta.url);
    const srcDir = path.join(path.dirname(thisFile), "..");

    const sourceFiles = (dir: string): string[] =>
      fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
        const entryPath = path.join(dir, entry.name);

        if (entry.isDirectory()) return sourceFiles(entryPath);

        return /\.(ts|tsx)$/.test(entry.name) && !/\.test\.(ts|tsx)$/.test(entry.name) ? [entryPath] : [];
      });

    const tauriFile = path.join(srcDir, "app", "tauri.ts");

    const importers = sourceFiles(srcDir).filter(
      (p) => p !== tauriFile && p !== thisFile && fs.readFileSync(p, "utf8").includes("@tauri-apps"),
    );

    expect(importers).toEqual([]);
  });
});
