import { describe, expect, it } from "vitest";
import type { RailNode } from "@contracts/agent/RailNode";
import type { Status } from "@contracts/Status";
import type { Actor } from "@contracts/Actor";
import type { EventData } from "@contracts/EventData";
import type { Event as DaemonEvent } from "@contracts/Event";
import type { TerminalInfo } from "@contracts/terminal/TerminalInfo";
import { connectEvents } from "../app/events";
import { createFakeApp } from "../testing/fakeApp";
import { createRailState } from "./rail";

const USER: Actor = { kind: "user", id: "you", parent: null };

const status = (label: string): Status => ({ kind: "working", label, since: 1 });

const node = (id: string, over: Partial<RailNode> = {}): RailNode => ({
  id,
  kind: "agent",
  name: id,
  parent: null,
  order: 0,
  status: status("starting"),
  meta: false,
  terminal_id: `t-${id}`,
  ...over,
});

const event = (data: EventData): DaemonEvent => ({ actor: USER, ...data });

const terminalInfo = (id: string, over: Partial<TerminalInfo> = {}): TerminalInfo => ({
  id,
  cwd: "/p",
  title: null,
  running: true,
  exit_code: null,
  ...over,
});

const open = async (tree: RailNode[], terminals: TerminalInfo[] = []) => {
  const app = createFakeApp();
  app.handlers["rail.tree"] = () => tree;
  app.handlers["terminal.list"] = () => terminals;
  const events = await connectEvents(app);
  const rail = createRailState(app, events);
  await rail.settled();

  return { app, rail };
};

describe("u3 Rail state", () => {
  it("u3_the_webview_subscribes_before_its_first_rail_tree_and_terminal_list", async () => {
    const order: string[] = [];
    const app = createFakeApp();
    app.handlers["rail.tree"] = () => (order.push("rail.tree"), []);
    app.handlers["terminal.list"] = () => (order.push("terminal.list"), []);
    const subscribe = app.subscribe;
    app.subscribe = async (listener) => {
      order.push("subscribe");

      return subscribe(listener);
    };

    const events = await connectEvents(app);
    await createRailState(app, events).settled();

    expect(order).toEqual(["subscribe", "rail.tree", "terminal.list"]);
  });

  it("u3_rail_changed_refetches_the_tree", async () => {
    const { app, rail } = await open([node("a")]);
    app.handlers["rail.tree"] = () => [node("a"), node("b")];

    app.emit(event({ name: "rail.changed" }));
    await rail.settled();

    expect(rail.nodes.map((each) => each.id)).toEqual(["a", "b"]);
  });

  it("u3_agent_status_replaces_one_nodes_status", async () => {
    const { app, rail } = await open([node("a"), node("b")]);

    app.emit(event({ name: "agent.status", data: { id: "b", status: status("editing") } }));

    expect(rail.nodes.map((each) => each.status?.label)).toEqual(["starting", "editing"]);
  });

  it("u3_terminal_exited_marks_the_node_with_that_terminal_id_exited_with_its_code", async () => {
    const { app, rail } = await open([node("a"), node("b")]);

    app.emit(event({ name: "terminal.exited", data: { id: "t-b", code: 3 } }));

    expect([rail.exitOf(node("a")), rail.exitOf(node("b"))]).toEqual([null, { code: 3 }]);
  });

  it("u3_terminal_exited_keeps_a_null_code_when_a_signal_ended_the_program", async () => {
    const { app, rail } = await open([node("a")]);

    app.emit(event({ name: "terminal.exited", data: { id: "t-a", code: null } }));

    expect(rail.exitOf(node("a"))).toEqual({ code: null });
  });

  it("u3_a_node_with_no_terminal_id_counts_as_exited", async () => {
    const { rail } = await open([]);

    expect(rail.exitOf(node("a", { terminal_id: null }))).toEqual({ code: null });
  });

  it("u3_a_terminal_that_the_first_terminal_list_reports_exited_is_exited", async () => {
    const { rail } = await open([node("a")], [terminalInfo("t-a", { running: false, exit_code: 1 })]);

    expect(rail.exitOf(node("a"))).toEqual({ code: 1 });
  });

  it("u3_a_group_is_never_exited", async () => {
    const { rail } = await open([]);

    expect(rail.exitOf(node("g", { kind: "group", status: null, terminal_id: null }))).toBeNull();
  });

  it("u3_events_during_an_in_flight_rail_tree_are_applied_after_it", async () => {
    const pending = Promise.withResolvers<RailNode[]>();
    const app = createFakeApp();
    app.handlers["rail.tree"] = () => pending.promise;
    const events = await connectEvents(app);
    const rail = createRailState(app, events);

    app.emit(event({ name: "agent.status", data: { id: "a", status: status("early") } }));
    pending.resolve([node("a")]);
    await rail.settled();

    expect(rail.nodes[0]?.status?.label).toBe("early");
  });

  it("u3_a_rail_changed_during_an_in_flight_rail_tree_refetches_after_it", async () => {
    const pending = Promise.withResolvers<RailNode[]>();
    let fetches = 0;
    const app = createFakeApp();
    app.handlers["rail.tree"] = () => (++fetches === 1 ? pending.promise : [node("a"), node("b")]);
    const events = await connectEvents(app);
    const rail = createRailState(app, events);

    app.emit(event({ name: "rail.changed" }));
    pending.resolve([node("a")]);
    await rail.settled();

    expect(rail.nodes.map((each) => each.id)).toEqual(["a", "b"]);
  });

  it("u3_no_node_is_selected_at_start", async () => {
    const { rail } = await open([node("a")]);

    expect(rail.selected()).toBeNull();
  });

  it("u3_selecting_a_node_deselects_the_one_before_it", async () => {
    const { rail } = await open([node("a"), node("b")]);

    rail.select("a");
    rail.select("b");

    expect(rail.selected()).toBe("b");
  });

  it("u3_a_node_that_leaves_the_tree_is_no_longer_selected", async () => {
    const { app, rail } = await open([node("a"), node("b")]);
    rail.select("b");
    app.handlers["rail.tree"] = () => [node("a")];

    app.emit(event({ name: "rail.changed" }));
    await rail.settled();

    expect(rail.selected()).toBeNull();
  });

  it("u3_name_of_the_user_is_you", async () => {
    const { rail } = await open([node("a", { name: "auth-refactor" })]);

    expect(rail.nameOf(USER)).toBe("you");
  });

  it("u3_name_of_an_agent_is_its_rail_name", async () => {
    const { rail } = await open([node("a", { name: "auth-refactor" })]);

    expect(rail.nameOf({ kind: "agent", id: "a", parent: null })).toBe("auth-refactor");
  });

  it.each([
    [{ kind: "agent", id: "gone", parent: null }],
    [{ kind: "ext", id: "linter", parent: null }],
  ] as const)("u3_name_of_%j_is_its_id", async (actor) => {
    const { rail } = await open([node("a")]);

    expect(rail.nameOf(actor)).toBe(actor.id);
  });
});
