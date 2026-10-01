import { Channel } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
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

afterEach(clearMocks);

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
});
