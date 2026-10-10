import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { RailNode } from "@contracts/agent/RailNode";
import { fakeEmulators } from "../terminal/paneHarness";
import { RpcError } from "../app/seam";
import { glyphOf, mountRail, railCallsTo, rowNames, rowOf } from "./railFixture";
import { agent, event, room, door, terminal } from "../testing/nodes";

afterEach(cleanup);

const SPAWNED = agent("fresh", "idle", "starting");

type Mounted = Awaited<ReturnType<typeof mountRail>>;

const answerWith = (mounted: Mounted, method: "agent.spawn" | "rail.spawnTerminal" | "rail.createRoom") => {
  mounted.app.handlers[method] = () => SPAWNED;
};

describe("u9 actions", () => {
  it("u9_plus_agent_spawns_at_the_top_level_with_no_selection", async () => {
    const mounted = await mountRail([room("g")]);
    answerWith(mounted, "agent.spawn");

    fireEvent.click(screen.getByText("agent"));

    await waitFor(() => expect(railCallsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: null, parent: null }]));
  });

  it("u9_plus_agent_spawns_under_the_selected_group", async () => {
    const mounted = await mountRail([room("g")]);
    answerWith(mounted, "agent.spawn");
    mounted.rail.select("g");

    fireEvent.click(screen.getByText("agent"));

    await waitFor(() => expect(railCallsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: null, parent: "g" }]));
  });

  it("u9_plus_agent_spawns_under_the_selected_meta_agent", async () => {
    const mounted = await mountRail([door("lead", "idle", "i")]);
    answerWith(mounted, "agent.spawn");
    mounted.rail.select("lead");

    fireEvent.click(screen.getByText("agent"));

    await waitFor(() =>
      expect(railCallsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: null, parent: "lead" }]),
    );
  });

  it.each<[string, RailNode]>([
    ["an agent", agent("a", "idle", "i")],
    ["a terminal", terminal("a")],
  ])("u9_plus_agent_with_%s_selected_spawns_at_the_top_level", async (_name, selected) => {
    const mounted = await mountRail([selected]);
    answerWith(mounted, "agent.spawn");
    mounted.rail.select("a");

    fireEvent.click(screen.getByText("agent"));

    await waitFor(() => expect(railCallsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: null, parent: null }]));
  });

  it("u9_the_new_agents_row_becomes_selected_once_the_tree_has_it", async () => {
    const tree: RailNode[] = [room("g")];
    const mounted = await mountRail(tree);
    mounted.app.handlers["agent.spawn"] = () => {
      tree.push(SPAWNED);
      mounted.app.emit(event({ name: "rail.changed" }));

      return SPAWNED;
    };

    fireEvent.click(screen.getByText("agent"));

    await waitFor(() => expect(mounted.rail.selected()).toBe("fresh"));
  });

  it("u9_the_webview_never_adds_a_row_the_tree_does_not_have", async () => {
    const mounted = await mountRail([room("g")]);
    answerWith(mounted, "agent.spawn");

    fireEvent.click(screen.getByText("agent"));
    await waitFor(() => expect(railCallsTo(mounted.app, "agent.spawn")).toHaveLength(1));

    expect([rowNames(), mounted.rail.selected()]).toEqual([["g"], null]);
  });

  it("u9_plus_terminal_spawns_a_terminal_under_the_selected_group", async () => {
    const tree: RailNode[] = [room("g")];
    const mounted = await mountRail(tree);
    mounted.app.handlers["rail.spawnTerminal"] = () => {
      tree.push(SPAWNED);
      mounted.app.emit(event({ name: "rail.changed" }));

      return SPAWNED;
    };

    mounted.rail.select("g");

    fireEvent.click(screen.getByText("terminal"));

    await waitFor(() => expect(railCallsTo(mounted.app, "rail.spawnTerminal")).toEqual([{ cwd: "/p", parent: "g" }]));
    await waitFor(() => expect(mounted.rail.selected()).toBe("fresh"));
  });

  it("u9_plus_terminal_spawns_at_the_top_level_with_no_selection", async () => {
    const tree: RailNode[] = [room("g")];
    const mounted = await mountRail(tree);
    mounted.app.handlers["rail.spawnTerminal"] = () => {
      tree.push(SPAWNED);
      mounted.app.emit(event({ name: "rail.changed" }));

      return SPAWNED;
    };

    fireEvent.click(screen.getByText("terminal"));

    await waitFor(() => expect(railCallsTo(mounted.app, "rail.spawnTerminal")).toEqual([{ cwd: "/p", parent: null }]));
    await waitFor(() => expect(mounted.rail.selected()).toBe("fresh"));
  });

  it("u9_plus_room_creates_a_root_room_even_with_a_selected_room", async () => {
    const mounted = await mountRail([room("g")]);
    answerWith(mounted, "rail.createRoom");
    mounted.rail.select("g");

    fireEvent.click(screen.getByText("room"));

    await waitFor(() => expect(railCallsTo(mounted.app, "rail.createRoom")).toEqual([{ name: "room", parent: null }]));
  });

  it("u9_double_clicking_a_name_edits_it_and_enter_renames", async () => {
    const mounted = await mountRail([agent("a", "idle", "i")]);
    mounted.app.handlers["rail.rename"] = () => agent("a", "idle", "i", { name: "renamed" });

    fireEvent.dblClick(screen.getByText("a"));
    const field = screen.getByLabelText("name");
    fireEvent.input(field, { target: { value: "renamed" } });
    fireEvent.keyDown(field, { key: "Enter" });

    await waitFor(() => expect(railCallsTo(mounted.app, "rail.rename")).toEqual([{ id: "a", name: "renamed" }]));
  });

  it("u9_escape_keeps_the_old_name_and_calls_nothing", async () => {
    const mounted = await mountRail([agent("a", "idle", "i")]);

    fireEvent.dblClick(screen.getByText("a"));
    const field = screen.getByLabelText("name");
    fireEvent.input(field, { target: { value: "renamed" } });
    fireEvent.keyDown(field, { key: "Escape" });

    expect([railCallsTo(mounted.app, "rail.rename"), screen.queryByLabelText("name"), rowNames()]).toEqual([[], null, ["a"]]);
  });

  it("u9_an_empty_name_keeps_the_old_name_and_calls_nothing", async () => {
    const mounted = await mountRail([agent("a", "idle", "i")]);

    fireEvent.dblClick(screen.getByText("a"));
    const field = screen.getByLabelText("name");
    fireEvent.input(field, { target: { value: "   " } });
    fireEvent.keyDown(field, { key: "Enter" });

    expect([railCallsTo(mounted.app, "rail.rename"), screen.queryByLabelText("name")]).toEqual([[], null]);
  });

  it("u9_hovering_a_plain_group_shows_promote_which_calls_rail_promote", async () => {
    const mounted = await mountRail([room("g")]);
    mounted.app.handlers["rail.startDoor"] = () => door("g", "idle", "i");

    fireEvent.mouseEnter(rowOf("g"));
    fireEvent.click(screen.getByText("start Door"));

    await waitFor(() => expect(railCallsTo(mounted.app, "rail.startDoor")).toEqual([{ id: "g" }]));
  });

  it("u9_promote_is_hidden_until_hover_and_never_on_an_agent", async () => {
    await mountRail([room("g"), agent("a", "idle", "i")]);
    const before = screen.queryByText("start Door");

    fireEvent.mouseEnter(rowOf("a"));

    expect([before, screen.queryByText("start Door")]).toEqual([null, null]);
  });

  it("u9_a_failed_call_shows_one_ink_line_until_the_next_click", async () => {
    const mounted = await mountRail([room("g")]);
    mounted.app.handlers["agent.spawn"] = () => {
      throw new RpcError(-32000, "no claude on PATH");
    };

    fireEvent.click(screen.getByText("agent"));
    const shown = await screen.findByText("no claude on PATH");
    fireEvent.click(rowOf("g"));

    expect([shown.className, screen.queryByText("no claude on PATH")]).toEqual(["ink", null]);
  });

  it("u9_the_failure_line_sits_above_the_actions", async () => {
    const mounted = await mountRail([]);
    mounted.app.handlers["rail.createRoom"] = () => {
      throw new RpcError(-32000, "boom");
    };

    fireEvent.click(screen.getByText("room"));
    const line = await screen.findByText("boom");

    expect(line.compareDocumentPosition(screen.getByText("agent")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("u9_the_failure_line_clears_on_a_click_on_the_fold_triangle", async () => {
    const mounted = await mountRail([room("g")]);
    mounted.app.handlers["rail.createRoom"] = () => {
      throw new RpcError(-32000, "boom");
    };

    fireEvent.click(screen.getByText("room"));
    await screen.findByText("boom");

    fireEvent.click(glyphOf("g"));

    expect(screen.queryByText("boom")).toBeNull();
  });

  it("u9_the_failure_line_clears_on_a_click_on_promote", async () => {
    const mounted = await mountRail([room("g")]);
    mounted.app.handlers["rail.createRoom"] = () => {
      throw new RpcError(-32000, "boom");
    };

    mounted.app.handlers["rail.startDoor"] = () => door("g", "idle", "i");

    fireEvent.click(screen.getByText("room"));
    await screen.findByText("boom");
    fireEvent.mouseEnter(rowOf("g"));

    fireEvent.click(screen.getByText("start Door"));

    expect(screen.queryByText("boom")).toBeNull();
  });

  it("u9_the_failure_line_clears_on_a_click_outside_the_rail", async () => {
    const mounted = await mountRail([]);
    mounted.app.handlers["rail.createRoom"] = () => {
      throw new RpcError(-32000, "boom");
    };

    fireEvent.click(screen.getByText("room"));
    await screen.findByText("boom");

    fireEvent.click(document.body);

    expect(screen.queryByText("boom")).toBeNull();
  });

  it("u9_promote_is_not_shown_on_a_meta_agent", async () => {
    await mountRail([door("lead", "idle", "i")]);

    fireEvent.mouseEnter(rowOf("lead"));

    expect(screen.queryByText("start Door")).toBeNull();
  });

  it("u9_a_collapsed_group_can_still_be_promoted", async () => {
    const mounted = await mountRail([room("g"), agent("a", "idle", "i", { parent: "g" })]);
    mounted.app.handlers["rail.startDoor"] = () => door("g", "idle", "i");
    fireEvent.click(glyphOf("g"));

    fireEvent.mouseEnter(rowOf("g"));
    fireEvent.click(screen.getByText("start Door"));

    await waitFor(() => expect(railCallsTo(mounted.app, "rail.startDoor")).toEqual([{ id: "g" }]));
  });

  it("u9_a_double_click_on_plus_agent_spawns_one_agent", async () => {
    const mounted = await mountRail([]);
    let finish: (node: RailNode) => void = () => {};

    mounted.app.handlers["agent.spawn"] = () => new Promise<RailNode>((done) => (finish = done));
    const button = screen.getByText("agent");

    fireEvent.click(button);
    fireEvent.click(button);
    finish(SPAWNED);

    await waitFor(() => expect(button).toHaveProperty("disabled", false));
    expect(railCallsTo(mounted.app, "agent.spawn")).toHaveLength(1);
  });

  it("u9_plus_terminal_and_plus_group_are_disabled_while_a_spawn_is_pending", async () => {
    const mounted = await mountRail([]);
    let finish: (node: RailNode) => void = () => {};

    mounted.app.handlers["rail.spawnTerminal"] = () => new Promise<RailNode>((done) => (finish = done));
    const terminalButton = screen.getByText("terminal");

    fireEvent.click(terminalButton);

    expect([terminalButton, screen.getByText("room")].map((button) => button.hasAttribute("disabled"))).toEqual([
      true,
      true,
    ]);
    finish(SPAWNED);
  });
});

it("u62_creates_one_root_room_selects_the_tree_row_and_starts_only_its_door", async () => {
  const tree: RailNode[] = [];
  const mounted = await mountRail(tree);
  mounted.app.handlers["rail.createRoom"] = () => room("returned-room");
  mounted.app.handlers["rail.startDoor"] = () => door("returned-room", "working", "starting");
  fireEvent.click(screen.getByText("room"));
  await waitFor(() => expect(railCallsTo(mounted.app, "rail.createRoom")).toEqual([{ name: "room", parent: null }]));
  expect(railCallsTo(mounted.app, "rail.startDoor")).toEqual([]);
  tree.push(room("returned-room"));
  mounted.app.emit(event({ name: "rail.changed" }));
  await waitFor(() => expect(mounted.rail.selected()).toBe("returned-room"));
  await waitFor(() => expect(railCallsTo(mounted.app, "rail.startDoor")).toEqual([{ id: "returned-room" }]));
});


it("u62_failed_start_retries_the_same_room_and_focuses_its_terminal", async () => {
  const tree: RailNode[] = [];
  const emulators = fakeEmulators();
  const mounted = await mountRail(tree, [], () => null, () => false, { pane: emulators.factory });
  mounted.app.handlers["rail.createRoom"] = () => {
    const created = room("actual");
    tree.push(created);
    mounted.app.emit(event({ name: "rail.changed" }));

    return created;
  };

  mounted.app.handlers["rail.startDoor"] = () => { throw new RpcError(-32003, "launch failed"); };

  fireEvent.click(screen.getByText("room"));
  await waitFor(() => expect(mounted.rail.selected()).toBe("actual"));
  await waitFor(() => expect(mounted.rail.doorFailure("actual")).toBe("launch failed"));
  expect(tree).toHaveLength(1);
  let finish: ((node: RailNode) => void) | undefined;
  mounted.app.handlers["rail.startDoor"] = () => new Promise<RailNode>((resolve) => { finish = resolve; });
  const retry = screen.getAllByText("retry Door")[0]!;
  fireEvent.click(retry);
  fireEvent.click(retry);
  await waitFor(() => expect(railCallsTo(mounted.app, "rail.startDoor")).toHaveLength(2));
  expect(retry.hasAttribute("disabled")).toBe(true);
  const live = door("actual", "working", "starting");
  tree.splice(0, 1, live);
  finish!(live);
  await waitFor(() => expect(document.activeElement?.getAttribute("data-terminal")).toBe("t-actual"));
  expect(railCallsTo(mounted.app, "rail.createRoom")).toHaveLength(1);
  expect(railCallsTo(mounted.app, "rail.startDoor")).toEqual([{id:"actual"},{id:"actual"}]);
});

it("u9_selecting_a_reopened_room_offers_start_without_launching", async () => {
  const mounted = await mountRail([room("reopened", {attempt:"4"})]);
  mounted.rail.select("reopened");
  expect(screen.getByText("start Door")).toBeDefined();
  expect(railCallsTo(mounted.app, "rail.startDoor")).toEqual([]);
  expect(railCallsTo(mounted.app, "rail.createRoom")).toEqual([]);
});

it("a24_a_pending_start_keeps_the_rail_interactive_and_the_pane_size", async () => {
  const mounted = await mountRail([room("closed", { attempt: "2" }), room("other")]);
  mounted.app.handlers["rail.startDoor"] = () => new Promise<RailNode>(() => {});
  mounted.rail.select("closed");
  const pane = document.querySelector<HTMLElement>(".pane");
  const before = pane?.getAttribute("style") ?? null;

  fireEvent.click(screen.getByText("start Door"));
  await waitFor(() => expect(mounted.rail.doorPending("closed")).toBe(true));
  fireEvent.click(screen.getByText("other"));

  expect(mounted.rail.selected()).toBe("other");
  expect(document.querySelector<HTMLElement>(".pane")?.getAttribute("style") ?? null).toBe(before);
});

it("a24_a_late_failure_shows_the_error_and_retry_door", async () => {
  const mounted = await mountRail([room("closed", { attempt: "2" })]);
  mounted.app.handlers["rail.startDoor"] = () => { throw new RpcError(-32003, "closed: no acknowledgement within 8s"); };

  mounted.rail.select("closed");

  fireEvent.click(screen.getByText("start Door"));

  await waitFor(() => expect(mounted.rail.doorFailure("closed")).toBe("closed: no acknowledgement within 8s"));
  expect(screen.getAllByText("retry Door").length).toBeGreaterThan(0);
  expect(mounted.rail.doorPending("closed")).toBe(false);
});

it("a24_a_start_the_daemon_never_answers_fails_within_the_bound", async () => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });

  try {
    const mounted = await mountRail([room("closed", { attempt: "2" })]);
    mounted.app.handlers["rail.startDoor"] = () => new Promise<RailNode>(() => {});
    mounted.rail.select("closed");

    fireEvent.click(screen.getByText("start Door"));
    await vi.advanceTimersByTimeAsync(10_000);

    await waitFor(() => expect(mounted.rail.doorFailure("closed")).not.toBeNull());
    expect(mounted.rail.doorPending("closed")).toBe(false);
  } finally {
    vi.useRealTimers();
  }
});

it("a24_a_start_that_succeeds_shows_the_doors_terminal", async () => {
  const tree: RailNode[] = [room("closed", { attempt: "2" })];
  const mounted = await mountRail(tree, [], () => null, () => false, { pane: fakeEmulators().factory });
  mounted.app.handlers["rail.startDoor"] = () => {
    const live = door("closed", "working", "starting");
    tree.splice(0, 1, live);

    return live;
  };

  mounted.rail.select("closed");

  fireEvent.click(screen.getAllByText("start Door")[0]!);

  await waitFor(() => expect(document.querySelector('[data-terminal="t-closed"]')).not.toBeNull());
  expect(mounted.rail.doorFailure("closed")).toBeNull();
});
