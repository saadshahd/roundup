import { describe, expect, it } from "vitest";
import { agent, room } from "../testing/nodes";
import { railStep } from "./railStep";

describe("u41 rail by keyboard, the rest: railStep", () => {
  it("u41_left_on_an_expanded_group_collapses_it", () => {
    const nodes = [room("g")];

    expect(railStep(nodes, "g", "left", true, undefined)).toEqual({ kind: "collapse" });
  });

  it("u41_left_on_a_collapsed_group_selects_its_parent", () => {
    const nodes = [room("outer"), room("inner", { parent: "outer" })];

    expect(railStep(nodes, "inner", "left", false, undefined)).toEqual({ kind: "select", id: "outer" });
  });

  it("u41_left_on_a_leaf_selects_its_parent", () => {
    const nodes = [room("g"), agent("a", "idle", "x", { parent: "g" })];

    expect(railStep(nodes, "a", "left", null, undefined)).toEqual({ kind: "select", id: "g" });
  });

  it("u41_left_at_depth_three_selects_the_immediate_parent_not_the_root", () => {
    const nodes = [
      room("outer"),
      room("inner", { parent: "outer" }),
      agent("leaf", "idle", "x", { parent: "inner" }),
    ];

    expect(railStep(nodes, "leaf", "left", null, undefined)).toEqual({ kind: "select", id: "inner" });
  });

  it("u41_left_on_a_top_level_leaf_does_nothing", () => {
    const nodes = [agent("a", "idle", "x")];

    expect(railStep(nodes, "a", "left", null, undefined)).toEqual({ kind: "none" });
  });

  it("u41_right_on_a_collapsed_group_expands_it", () => {
    const nodes = [room("g")];

    expect(railStep(nodes, "g", "right", false, undefined)).toEqual({ kind: "expand" });
  });

  it("u41_right_on_an_expanded_group_selects_its_first_child", () => {
    const nodes = [
      room("g"),
      agent("b", "idle", "x", { parent: "g", order: 1 }),
      agent("a", "idle", "x", { parent: "g", order: 0 }),
    ];

    expect(railStep(nodes, "g", "right", true, "a")).toEqual({ kind: "select", id: "a" });
  });

  it("u41_right_on_a_meta_agent_with_no_toggle_selects_its_first_child", () => {
    const nodes = [room("m", { attempt: "1" }), agent("a", "idle", "x", { parent: "m" })];

    expect(railStep(nodes, "m", "right", null, "a")).toEqual({ kind: "select", id: "a" });
  });

  it("u41_right_on_a_leaf_with_no_children_does_nothing", () => {
    const nodes = [agent("a", "idle", "x")];

    expect(railStep(nodes, "a", "right", null, undefined)).toEqual({ kind: "none" });
  });
});
