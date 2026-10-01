import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RailNode } from "@contracts/agent/RailNode";
import { RpcError } from "../app/seam";
import { dragFrom, pointerAt, release, stubLayout } from "./dragFixture";
import { agent, callsTo, group, MINUTE, mountRail, NOW, rowNames, rowOf } from "./railFixture";

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
    const done = agent("d", "done", "finished", {
      order: 0,
      status: { kind: "done", label: "finished", since: NOW - 60 * MINUTE },
    });

    const mounted = await mountRail([
      done,
      agent("a", "idle", "i", { order: 1 }),
      agent("b", "idle", "i", { order: 2 }),
      agent("c", "idle", "i", { order: 3 }),
    ]);

    mounted.app.handlers["rail.move"] = () => null;
    fireEvent.click(screen.getByText("✓ 1 done"));

    dragFrom("a", pointerAt(4, 0));
    release(pointerAt(4, 0));

    await waitFor(() => expect(callsTo(mounted.app, "rail.move")).toEqual([{ id: "a", parent: null, index: 3 }]));
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
    await mountRail(TREE, [], () => true);

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
