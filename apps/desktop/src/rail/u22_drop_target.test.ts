import { describe, expect, it } from "vitest";
import type { RailNode } from "@contracts/agent/RailNode";
import { dropAt, isInPlace, remainingRows } from "./dragTarget";
import { layoutRail } from "./layout";
import type { NodeRow } from "./layout";
import { agent, group, MINUTE, NOW } from "../testing/nodes";

/** a, g { x, y }, b */
const TREE: RailNode[] = [
  agent("a", "idle", "i", { order: 0 }),
  group("g", { order: 1 }),
  agent("x", "idle", "i", { parent: "g", order: 0 }),
  agent("y", "idle", "i", { parent: "g", order: 1 }),
  agent("b", "idle", "i", { order: 2 }),
];

const rowsOf = (nodes: RailNode[], collapsed: string[] = [], unfolded: (string | null)[] = [], dragged: string | null = null): NodeRow[] =>
  layoutRail(nodes, { collapsed: new Set(collapsed), unfolded: new Set(unfolded), now: NOW, dragged }).filter(
    (row): row is NodeRow => row.kind === "node",
  );

const dropOf = (dragged: string, slot: number, depth: number, nodes = TREE, collapsed: string[] = []) =>
  dropAt(nodes, remainingRows(nodes, rowsOf(nodes, collapsed), dragged), dragged, slot, depth);

/** Displayed `a b c d` once the done line is open, although the Daemon's order is d, a, b, c. */
const WITH_DONE: RailNode[] = [
  agent("d", "done", "finished", { order: 0, status: { kind: "done", label: "finished", since: NOW - 60 * MINUTE } }),
  agent("a", "idle", "i", { order: 1 }),
  agent("b", "idle", "i", { order: 2 }),
  agent("c", "idle", "i", { order: 3 }),
];

describe("u22 drop target", () => {
  it("u22_an_opened_done_line_is_closed_while_a_row_is_dragged", () => {
    const rows = rowsOf(WITH_DONE, [], [null], "a");

    expect(remainingRows(WITH_DONE, rows, "a").map((row) => row.node.id)).toEqual(["b", "c"]);
  });

  it("u22_a_dragged_done_row_is_shown_at_its_place_in_order", () => {
    expect(rowsOf(WITH_DONE, [], [null], "d").map((row) => row.node.id)).toEqual(["d", "a", "b", "c"]);
  });

  it("u22_below_the_last_live_row_a_drop_counts_the_done_rows_in_the_index", () => {
    const rows = remainingRows(WITH_DONE, rowsOf(WITH_DONE, [], [null], "a"), "a");

    expect(dropAt(WITH_DONE, rows, "a", 2, 0)).toEqual({ parent: null, index: 3, depth: 0 });
  });

  it("u22_above_the_first_row_lands_first_at_the_top_level", () => {
    expect(dropOf("b", 0, 3)).toEqual({ parent: null, index: 0, depth: 0 });
  });

  it("u22_below_an_agent_the_depth_cannot_nest_under_it", () => {
    expect(dropOf("b", 1, 1)).toEqual({ parent: null, index: 1, depth: 0 });
  });

  it("u22_between_a_group_and_its_first_child_lands_first_in_the_group_at_any_depth", () => {
    expect(dropOf("b", 2, 0)).toEqual({ parent: "g", index: 0, depth: 1 });
  });

  it("u22_between_two_children_lands_between_them", () => {
    expect(dropOf("b", 3, 0)).toEqual({ parent: "g", index: 1, depth: 1 });
  });

  it("u22_below_the_last_child_a_deeper_pointer_lands_in_the_group", () => {
    expect(dropOf("b", 4, 1)).toEqual({ parent: "g", index: 2, depth: 1 });
  });

  it("u22_below_the_last_child_a_shallower_pointer_lands_beside_the_group", () => {
    expect(dropOf("b", 4, 0)).toEqual({ parent: null, index: 2, depth: 0 });
  });

  it("u22_a_pointer_further_right_than_any_row_nests_under_an_empty_group", () => {
    const nodes = [group("g"), agent("a", "idle", "i", { order: 1 })];

    expect(dropOf("a", 1, 5, nodes)).toEqual({ parent: "g", index: 0, depth: 1 });
  });

  it("u22_a_collapsed_group_cannot_be_nested_under", () => {
    expect(dropOf("b", 2, 5, TREE, ["g"])).toEqual({ parent: null, index: 2, depth: 0 });
  });

  it("u22_a_meta_agent_can_be_nested_under", () => {
    const nodes = [group("m", { meta: true }), agent("a", "idle", "i", { order: 1 })];

    expect(dropOf("a", 1, 5, nodes)).toEqual({ parent: "m", index: 0, depth: 1 });
  });

  it("u22_the_dragged_rows_descendants_are_not_places_to_drop", () => {
    const remaining = remainingRows(TREE, rowsOf(TREE), "g").map((row) => row.node.id);

    expect(remaining).toEqual(["a", "b"]);
  });

  it("u22_the_index_counts_siblings_without_the_dragged_row", () => {
    expect(dropOf("a", 4, 0)).toEqual({ parent: null, index: 2, depth: 0 });
  });

  it("u22_a_drop_where_the_row_already_is_is_in_place", () => {
    expect(isInPlace(TREE, "b", dropOf("b", 4, 0))).toBe(true);
  });

  it("u22_a_drop_elsewhere_is_not_in_place", () => {
    expect(isInPlace(TREE, "b", dropOf("b", 0, 0))).toBe(false);
  });
});
