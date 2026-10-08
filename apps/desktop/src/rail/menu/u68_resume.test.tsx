import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import type { RailNode } from "@contracts/agent/RailNode";
import { RpcError } from "../../app/seam";
import { agent, door, event, room, terminal } from "../../testing/nodes";
import { fakeEmulators } from "../../terminal/paneHarness";
import { exitedTerminal, mountRail, railCallsTo, rowOf } from "../railFixture";

afterEach(cleanup);

const openAt = (name: string) => fireEvent.contextMenu(rowOf(name), { clientX: 120, clientY: 80 });

const labels = () => screen.getAllByRole("menuitem").map((each) => each.textContent);

const exited = (id: string) => exitedTerminal(id, 0);

const resumable = (id: string, over: Partial<RailNode> = {}) => agent(id, "done", "done", { can_resume: true, ...over });

describe("u68 resume from the Rail menu", () => {
  it("u68_an_exited_resumable_agent_offers_resume_then_remove", async () => {
    await mountRail([resumable("a")], [exited("a")]);

    openAt("a");

    expect(labels()).toEqual(["resume", "remove"]);
  });

  it("u68_menu_per_row_kind", async () => {
    await mountRail(
      [
        resumable("live-done"),
        agent("live-error", "error", "failed"),
        agent("gone", "done", "done"),
        resumable("run", { status: { kind: "working", label: "busy", since: 0 } }),
        room("g"),
        door("d", "done", "done", { can_resume: true }),
        terminal("t"),
      ],
      [exited("gone"), exited("d")],
    );

    openAt("live-done");
    expect(labels()).toEqual(["stop", "remove"]);
    openAt("live-error");
    expect(labels()).toEqual(["stop", "remove"]);
    openAt("gone");
    expect(labels()).toEqual(["remove"]);
    openAt("run");
    expect(labels()).toEqual(["stop", "remove"]);
    openAt("g");
    expect(labels()).toEqual(["remove"]);
    openAt("d");
    expect(labels()).toEqual(["resume", "remove"]);
    openAt("t");
    expect(labels()).toEqual(["stop", "remove"]);
  });

  it("u68_the_menu_follows_an_exit_without_reload", async () => {
    const { app } = await mountRail([resumable("a")]);

    openAt("a");
    expect(labels()).toEqual(["stop", "remove"]);

    app.emit(event({ name: "terminal.exited", data: { id: "t-a", code: 0 } }));
    await waitFor(() => {
      openAt("a");
      expect(labels()).toEqual(["resume", "remove"]);
    });
  });

  it("u68_pointer_and_keyboard_send_one_agent_resume_and_nothing_else", async () => {
    const { app } = await mountRail([resumable("a"), resumable("b")], [exited("a"), exited("b")]);

    app.handlers["agent.resume"] = () => resumable("a");
    openAt("a");
    fireEvent.click(screen.getByRole("menuitem", { name: "resume" }));
    expect(screen.queryByRole("menu")).toBeNull();

    openAt("b");
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Enter" });

    await waitFor(() => expect(railCallsTo(app, "agent.resume")).toEqual([{ id: "a" }, { id: "b" }]));

    for (const method of ["agent.spawn", "rail.spawnTerminal", "terminal.write"]) expect(railCallsTo(app, method)).toEqual([]);
  });

  it("u68_a_second_activation_while_pending_sends_nothing", async () => {
    const { app } = await mountRail([resumable("a")], [exited("a")]);
    let finish: (node: RailNode) => void = () => {};

    app.handlers["agent.resume"] = () => new Promise<RailNode>((resolve) => { finish = resolve; });
    openAt("a");
    fireEvent.click(screen.getByRole("menuitem", { name: "resume" }));
    openAt("a");
    fireEvent.click(screen.getByRole("menuitem", { name: "resume" }));

    expect(railCallsTo(app, "agent.resume")).toEqual([{ id: "a" }]);
    finish(resumable("a"));
  });

  it("u68_success_refetches_and_binds_a_new_emulator_in_place", async () => {
    const tree: RailNode[] = [resumable("a"), agent("other", "working", "busy")];
    const emulators = fakeEmulators();
    const { app, rail } = await mountRail(tree, [exited("a")], () => null, () => false, { pane: emulators.factory });

    app.handlers["terminal.snapshot"] = () => ({ data: "", cols: 100, rows: 30, after: 0 });
    app.handlers["terminal.resize"] = () => null;
    rail.select("a");
    await waitFor(() => expect(emulators.made.has("t-a")).toBe(true));
    app.handlers["agent.resume"] = () => {
      tree[0] = agent("a", "working", "starting", { terminal_id: "t-a2" });

      return tree[0];
    };

    openAt("a");
    fireEvent.click(screen.getByRole("menuitem", { name: "resume" }));

    await waitFor(() => expect(emulators.made.has("t-a2")).toBe(true));
    expect(rail.nodes.map((node) => node.id)).toEqual(["a", "other"]);
    expect(rail.selected()).toBe("a");
    expect(emulators.made.get("t-a2")).not.toBe(emulators.made.get("t-a"));
    expect(emulators.made.get("t-a")?.disposed).toBe(false);
  });

  it("u68_success_after_selecting_another_row_moves_neither_selection_nor_focus", async () => {
    const tree: RailNode[] = [resumable("a"), agent("other", "working", "busy")];
    const { app, rail } = await mountRail(tree, [exited("a")]);
    let finish: (node: RailNode) => void = () => {};

    app.handlers["agent.resume"] = () => new Promise<RailNode>((resolve) => { finish = resolve; });
    openAt("a");
    fireEvent.click(screen.getByRole("menuitem", { name: "resume" }));
    rail.select("other");
    rowOf("other").focus();
    tree[0] = agent("a", "working", "starting", { terminal_id: "t-a2" });
    finish(tree[0]);

    await waitFor(() => expect(railCallsTo(app, "rail.tree").length).toBeGreaterThan(1));
    await rail.settled();
    expect(rail.selected()).toBe("other");
    expect(document.activeElement).toBe(rowOf("other"));
  });

  it("u68_a_rejection_keeps_the_row_and_screen_and_shows_one_failure_line", async () => {
    const emulators = fakeEmulators();
    const { app, rail } = await mountRail([resumable("a")], [exited("a")], () => null, () => false, { pane: emulators.factory });

    app.handlers["terminal.snapshot"] = () => ({ data: "", cols: 100, rows: 30, after: 0 });
    app.handlers["terminal.resize"] = () => null;
    rail.select("a");
    app.handlers["agent.resume"] = () => { throw new RpcError(-32003, "cannot start\nsecond line"); };

    openAt("a");
    fireEvent.click(screen.getByRole("menuitem", { name: "resume" }));

    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("cannot start"));
    expect(rail.selected()).toBe("a");
    expect(rail.nodes[0]?.terminal_id).toBe("t-a");
    expect(emulators.made.get("t-a")?.disposed).toBe(false);
  });

  it("u68_not_found_shows_the_line_and_refetches_the_tree", async () => {
    const tree: RailNode[] = [resumable("a")];
    const { app } = await mountRail(tree, [exited("a")]);

    app.handlers["agent.resume"] = () => { throw new RpcError(-32001, "no agent a"); };

    openAt("a");
    const before = railCallsTo(app, "rail.tree").length;
    fireEvent.click(screen.getByRole("menuitem", { name: "resume" }));

    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("no agent a"));
    await waitFor(() => expect(railCallsTo(app, "rail.tree").length).toBe(before + 1));
  });

  it("u68_unknown_outcome_is_never_retried_and_binds_the_terminal_the_tree_names", async () => {
    const tree: RailNode[] = [resumable("a")];
    const emulators = fakeEmulators();
    const { app, rail } = await mountRail(tree, [exited("a")], () => null, () => false, { pane: emulators.factory });

    app.handlers["terminal.snapshot"] = () => ({ data: "", cols: 100, rows: 30, after: 0 });
    app.handlers["terminal.resize"] = () => null;
    rail.select("a");
    app.handlers["agent.resume"] = () => {
      tree[0] = agent("a", "working", "starting", { terminal_id: "t-a2" });

      throw new RpcError(-32004, "the Daemon may have run the call");
    };

    const trees = railCallsTo(app, "rail.tree").length;
    const lists = railCallsTo(app, "terminal.list").length;
    openAt("a");
    fireEvent.click(screen.getByRole("menuitem", { name: "resume" }));

    await waitFor(() => expect(emulators.made.has("t-a2")).toBe(true));
    expect(railCallsTo(app, "agent.resume")).toEqual([{ id: "a" }]);
    expect(railCallsTo(app, "rail.tree").length).toBe(trees + 1);
    await waitFor(() => expect(railCallsTo(app, "terminal.list").length).toBe(lists + 1));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("u68_unknown_outcome_with_a_failed_refresh_says_the_outcome_is_unknown", async () => {
    const { app } = await mountRail([resumable("a")], [exited("a")]);

    app.handlers["agent.resume"] = () => {
      app.handlers["rail.tree"] = () => { throw new RpcError(-32000, "down"); };

      throw new RpcError(-32004, "the Daemon may have run the call");
    };

    openAt("a");
    fireEvent.click(screen.getByRole("menuitem", { name: "resume" }));

    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("outcome unknown"));
    expect(railCallsTo(app, "agent.resume")).toEqual([{ id: "a" }]);
  });

  it("u68_a_reload_before_and_after_resume_never_calls_agent_resume", async () => {
    const { app } = await mountRail([resumable("a")], [exited("a")]);

    expect(railCallsTo(app, "agent.resume")).toEqual([]);
    cleanup();
    const again = await mountRail([agent("a", "working", "starting", { terminal_id: "t-a2" })]);

    expect(railCallsTo(again.app, "agent.resume")).toEqual([]);
  });
});
