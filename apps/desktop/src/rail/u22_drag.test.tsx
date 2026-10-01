import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RailNode } from "@contracts/agent/RailNode";
import { RpcError } from "../app/seam";
import type { DaemonExit } from "../app/seam";
import { dragFrom, pointerAt, release, stubLayout } from "./dragFixture";
import { callsTo, mountRail, rowNames, rowOf } from "./railFixture";
import { agent, event, group, MINUTE, NOW } from "../testing/nodes";

/** a, g { x }, b */
const TREE: RailNode[] = [
  agent("a", "idle", "i", { order: 0 }),
  group("g", { order: 1 }),
  agent("x", "idle", "i", { parent: "g", order: 0 }),
  agent("b", "idle", "i", { order: 2 }),
];

beforeEach(() => {
  stubLayout();
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const doneAgent = (id: string, order: number, over: Partial<RailNode> = {}): RailNode =>
  agent(id, "done", "finished", { order, status: { kind: "done", label: "finished", since: NOW - 60 * MINUTE }, ...over });

const shiftsOf = () =>
  Object.fromEntries(
    screen
      .getAllByRole("treeitem")
      .filter((row) => row.dataset.shift !== "0")
      .map((row) => [row.dataset.id, row.dataset.shift]),
  );

const dropLine = () => document.querySelector<HTMLElement>(".drop-line");

describe("u22 drag", () => {
  it("u22_releasing_a_dragged_row_calls_rail_move_with_the_parent_and_index", async () => {
    const mounted = await mountRail(TREE);
    mounted.app.handlers["rail.move"] = () => null;

    dragFrom("b", pointerAt(0, 0));
    release(pointerAt(0, 0));

    await waitFor(() => expect(callsTo(mounted.app, "rail.move")).toEqual([{ id: "b", parent: null, index: 0 }]));
  });

  it("u22_moving_sideways_changes_the_depth_the_row_lands_at", async () => {
    const mounted = await mountRail(TREE);
    mounted.app.handlers["rail.move"] = () => null;

    dragFrom("b", pointerAt(3, 1));
    release(pointerAt(3, 1));

    await waitFor(() => expect(callsTo(mounted.app, "rail.move")).toEqual([{ id: "b", parent: "g", index: 1 }]));
  });

  it("u22_the_drop_line_starts_at_the_depth_the_row_lands_at", async () => {
    await mountRail(TREE);

    dragFrom("b", pointerAt(3, 1));

    expect(dropLine()?.style.getPropertyValue("--depth")).toBe("1");
  });

  it("u22_no_drop_line_shows_before_the_pointer_has_moved_past_a_click", async () => {
    await mountRail(TREE);

    fireEvent.pointerDown(rowOf("b"), { button: 0, clientX: 5, clientY: 5 });
    fireEvent.pointerMove(window, { clientX: 6, clientY: 6 });

    expect(dropLine()).toBeNull();
  });

  it("u22_the_rows_between_the_drop_line_and_the_dragged_row_shift_down_into_the_room", async () => {
    await mountRail(TREE);

    dragFrom("b", pointerAt(1, 0));

    expect(shiftsOf()).toEqual({ g: "1", x: "1" });
  });

  it("u22_the_rows_between_the_dragged_row_and_the_drop_line_shift_up_into_the_room_it_left", async () => {
    await mountRail(TREE);

    dragFrom("a", pointerAt(4, 0));

    expect(shiftsOf()).toEqual({ g: "-1", x: "-1", b: "-1" });
  });

  it("u22_a_drop_below_an_opened_done_line_lands_where_the_line_is_drawn", async () => {
    const mounted = await mountRail([
      doneAgent("d", 0),
      agent("a", "idle", "i", { order: 1 }),
      agent("b", "idle", "i", { order: 2 }),
      agent("c", "idle", "i", { order: 3 }),
    ]);

    mounted.app.handlers["rail.move"] = () => null;
    fireEvent.click(screen.getByText("✓ 1 done"));

    dragFrom("a", pointerAt(4, 0));

    expect(dropLine()?.style.top).toBe("40px");

    release(pointerAt(4, 0));

    await waitFor(() => expect(callsTo(mounted.app, "rail.move")).toEqual([{ id: "a", parent: null, index: 3 }]));
  });

  it("u22_a_drop_below_a_done_line_opened_inside_a_group_lands_where_the_line_is_drawn", async () => {
    const mounted = await mountRail([
      agent("a", "idle", "i", { order: 0 }),
      agent("b", "idle", "i", { order: 1 }),
      agent("c", "idle", "i", { order: 2 }),
      group("g", { order: 3 }),
      agent("x", "idle", "i", { parent: "g", order: 0 }),
      doneAgent("e", 1, { parent: "g" }),
      doneAgent("d", 4),
    ]);

    mounted.app.handlers["rail.move"] = () => null;
    screen.getAllByText("✓ 1 done").forEach((line) => fireEvent.click(line));

    dragFrom("c", pointerAt(6, 0));

    expect(dropLine()?.style.top).toBe("80px");

    release(pointerAt(6, 0));

    await waitFor(() => expect(callsTo(mounted.app, "rail.move")).toEqual([{ id: "c", parent: null, index: 3 }]));
  });

  it("u22_a_done_row_can_itself_be_dragged", async () => {
    const mounted = await mountRail([
      agent("a", "idle", "i", { order: 0 }),
      doneAgent("d", 1),
      agent("b", "idle", "i", { order: 2 }),
    ]);

    mounted.app.handlers["rail.move"] = () => null;
    fireEvent.click(screen.getByText("✓ 1 done"));

    dragFrom("d", pointerAt(0, 0));
    release(pointerAt(0, 0));

    await waitFor(() => expect(callsTo(mounted.app, "rail.move")).toEqual([{ id: "d", parent: null, index: 0 }]));
  });

  it("u22_the_drop_line_sits_where_the_pointer_is_when_the_row_moves_up", async () => {
    await mountRail(TREE);

    dragFrom("b", pointerAt(1, 0));

    expect(dropLine()?.style.top).toBe("20px");
  });

  it("u22_the_drop_line_sits_at_the_dragged_rows_own_place_when_the_pointer_has_not_left_it", async () => {
    await mountRail(TREE);

    dragFrom("b", pointerAt(3, 0));

    expect(dropLine()?.style.top).toBe("60px");
  });

  it("u22_a_pointer_just_below_a_rows_middle_lands_below_it", async () => {
    const mounted = await mountRail(TREE);
    mounted.app.handlers["rail.move"] = () => null;

    dragFrom("b", { clientX: 0, clientY: 11 });
    release({ clientX: 0, clientY: 11 });

    await waitFor(() => expect(callsTo(mounted.app, "rail.move")).toEqual([{ id: "b", parent: null, index: 1 }]));
  });

  it("u22_a_pointer_just_above_a_rows_middle_lands_above_it", async () => {
    const mounted = await mountRail(TREE);
    mounted.app.handlers["rail.move"] = () => null;

    dragFrom("b", { clientX: 0, clientY: 9 });
    release({ clientX: 0, clientY: 9 });

    await waitFor(() => expect(callsTo(mounted.app, "rail.move")).toEqual([{ id: "b", parent: null, index: 0 }]));
  });

  it("u22_a_tree_change_during_a_drag_ends_it_without_a_call", async () => {
    const tree = [...TREE];
    const mounted = await mountRail(tree);
    mounted.app.handlers["rail.tree"] = () => [...tree];

    dragFrom("b", pointerAt(0, 0));
    tree.unshift(agent("fresh", "idle", "i", { order: -1 }));
    mounted.app.emit(event({ name: "rail.changed" }));
    await mounted.rail.settled();

    expect(dropLine()).toBeNull();

    release(pointerAt(0, 0));

    expect(callsTo(mounted.app, "rail.move")).toEqual([]);
  });

  describe.each<[string, (mounted: Awaited<ReturnType<typeof mountRail>>) => void]>([
    ["a release", () => release(pointerAt(0, 0))],
    ["escape", () => fireEvent.keyDown(window, { key: "Escape" })],
    [
      "a conflict",
      (mounted) => {
        mounted.app.handlers["rail.move"] = () => {
          throw new RpcError(-32000, "CONFLICT: no");
        };

        release(pointerAt(0, 0));
      },
    ],
  ])("u22 after %s", (_ending, end) => {
    it("u22_the_opened_done_line_and_the_live_line_are_back", async () => {
      const mounted = await mountRail([doneAgent("d", 0), agent("b", "idle", "i", { order: 1 })]);
      mounted.rail.select("b");
      fireEvent.click(screen.getByText("✓ 1 done"));
      const before = [rowNames(), rowOf("b").querySelector(".live") !== null];

      dragFrom("b", pointerAt(0, 0));
      end(mounted);
      await waitFor(() => expect([rowNames(), rowOf("b").querySelector(".live") !== null]).toEqual(before));
    });
  });

  it("u22_live_lines_are_hidden_while_a_drag_is_under_way", async () => {
    await mountRail(TREE);
    fireEvent.mouseEnter(rowOf("b"));
    const before = rowOf("b").querySelector(".live");

    dragFrom("b", pointerAt(1, 0));

    expect([before === null, rowOf("b").querySelector(".live")]).toEqual([false, null]);
  });

  it("u22_the_drop_line_sits_at_the_edge_of_the_room_the_shifted_rows_opened", async () => {
    await mountRail(TREE);

    dragFrom("a", pointerAt(4, 0));

    expect(dropLine()?.style.top).toBe("60px");
  });

  it("u22_reduced_motion_turns_the_shift_transition_off", async () => {
    await mountRail(TREE, [], () => null, () => true);

    dragFrom("b", pointerAt(1, 0));

    expect(document.querySelector<HTMLElement>(".rail-tree")?.style.getPropertyValue("--shift-ms")).toBe("0ms");
  });

  it("u22_the_dragged_row_is_lifted_while_the_drag_lasts", async () => {
    await mountRail(TREE);

    dragFrom("b", pointerAt(1, 0));

    expect(rowOf("b").dataset.lifted).toBe("true");
  });

  it("u22_a_conflict_leaves_the_row_where_it_was_and_shows_the_message", async () => {
    const mounted = await mountRail(TREE);
    mounted.app.handlers["rail.move"] = () => {
      throw new RpcError(-32000, "CONFLICT: cannot move under an agent");
    };

    dragFrom("b", pointerAt(0, 0));
    release(pointerAt(0, 0));

    await screen.findByText("✕ CONFLICT: cannot move under an agent");
    expect([rowNames(), dropLine()]).toEqual([["a", "g", "x", "b"], null]);
  });

  it("u22_the_click_that_follows_a_release_does_not_clear_the_message", async () => {
    const mounted = await mountRail(TREE);
    mounted.app.handlers["rail.move"] = () => {
      throw new RpcError(-32000, "CONFLICT: cannot move under an agent");
    };

    dragFrom("b", pointerAt(0, 0));
    release(pointerAt(0, 0));
    await screen.findByText("✕ CONFLICT: cannot move under an agent");
    fireEvent.click(rowOf("a"));

    expect(screen.queryByText("✕ CONFLICT: cannot move under an agent")).not.toBeNull();
  });

  it("u22_releasing_where_the_row_already_is_calls_nothing", async () => {
    const mounted = await mountRail(TREE);

    dragFrom("b", pointerAt(3, 0));
    release(pointerAt(3, 0));

    expect([callsTo(mounted.app, "rail.move"), dropLine()]).toEqual([[], null]);
  });

  it("u22_no_drag_starts_once_the_daemon_has_exited", async () => {
    const mounted = await mountRail(TREE, [], () => ({ code: 1 }));

    dragFrom("b", pointerAt(0, 0));

    expect([dropLine(), rowOf("b").dataset.lifted]).toEqual([null, "false"]);

    release(pointerAt(0, 0));

    expect(callsTo(mounted.app, "rail.move")).toEqual([]);
  });

  it("u22_a_drag_under_way_when_the_daemon_exits_drops_nothing", async () => {
    const [exit, setExit] = createSignal<DaemonExit | null>(null);
    const mounted = await mountRail(TREE, [], exit);

    dragFrom("b", pointerAt(0, 0));
    setExit({ code: 1 });
    release(pointerAt(0, 0));

    expect(callsTo(mounted.app, "rail.move")).toEqual([]);
  });

  it("u22_escape_cancels_the_drag_and_calls_nothing", async () => {
    const mounted = await mountRail(TREE);

    dragFrom("b", pointerAt(0, 0));
    fireEvent.keyDown(window, { key: "Escape" });
    release(pointerAt(0, 0));

    expect([callsTo(mounted.app, "rail.move"), dropLine()]).toEqual([[], null]);
  });

  it("u22_a_press_that_never_moved_is_a_click_not_a_drag", async () => {
    const mounted = await mountRail(TREE);

    fireEvent.pointerDown(rowOf("b"), { button: 0, clientX: 5, clientY: 5 });
    release({ clientX: 5, clientY: 5 });
    fireEvent.click(rowOf("b"));

    expect([callsTo(mounted.app, "rail.move"), dropLine(), mounted.rail.selected()]).toEqual([[], null, "b"]);
  });

  it("u22_a_drag_cannot_start_on_a_button", async () => {
    const mounted = await mountRail(TREE);

    fireEvent.pointerDown(screen.getByRole("button", { name: "collapse" }), { button: 0, clientX: 5, clientY: 5 });
    fireEvent.pointerMove(window, pointerAt(0, 0));
    release(pointerAt(0, 0));

    expect([callsTo(mounted.app, "rail.move"), dropLine()]).toEqual([[], null]);
  });

  it("u22_a_group_dragged_past_its_own_children_lands_outside_it", async () => {
    const mounted = await mountRail(TREE);
    mounted.app.handlers["rail.move"] = () => null;

    dragFrom("g", pointerAt(4, 1));
    release(pointerAt(4, 1));

    await waitFor(() => expect(callsTo(mounted.app, "rail.move")).toEqual([{ id: "g", parent: null, index: 2 }]));
  });
});
