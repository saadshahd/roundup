import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import type { Kind } from "@contracts/Kind";
import type { RailNode } from "@contracts/agent/RailNode";
import { agent, glyphOf, group, metaAgent, MINUTE, mountRail, NOW, rowNames, rowOf, terminal } from "./railFixture";

afterEach(cleanup);

const doneFor = (id: string, minutes: number, over: Partial<RailNode> = {}): RailNode =>
  agent(id, "done", "finished", { status: { kind: "done", label: "finished", since: NOW - minutes * MINUTE }, ...over });

describe("u8 folding", () => {
  it("u8_agents_done_for_10_minutes_fold_into_one_line_after_their_siblings", async () => {
    await mountRail([
      doneFor("old-1", 10, { order: 0 }),
      doneFor("old-2", 90, { order: 1 }),
      agent("busy", "working", "w", { order: 2 }),
    ]);

    expect([rowNames(), screen.getByText("✓ 2 done").textContent]).toEqual([["busy"], "✓ 2 done"]);
  });

  it("u8_an_agent_done_for_under_10_minutes_stays_in_its_row", async () => {
    await mountRail([doneFor("fresh", 9, { order: 0 }), agent("busy", "working", "w", { order: 1 })]);

    expect([rowNames(), screen.queryByText(/done$/)]).toEqual([["fresh", "busy"], null]);
  });

  it("u8_an_agent_folds_once_the_clock_passes_10_minutes_of_done", async () => {
    const { setNow } = await mountRail([doneFor("fresh", 9)]);

    setNow(NOW + 2 * MINUTE);

    expect(screen.getByText("✓ 1 done")).toBeDefined();
  });

  it("u8_clicking_the_done_line_unfolds_its_agents", async () => {
    await mountRail([doneFor("old-1", 20, { order: 0 }), agent("busy", "working", "w", { order: 1 })]);

    fireEvent.click(screen.getByText("✓ 1 done"));

    expect([rowNames(), screen.queryByText("✓ 1 done")]).toEqual([["busy", "old-1"], null]);
  });

  it("u8_each_parent_folds_its_own_done_agents_at_its_own_depth", async () => {
    await mountRail([
      group("migrate"),
      doneFor("inner", 30, { parent: "migrate" }),
      doneFor("outer", 30, { order: 1 }),
    ]);

    expect(screen.getAllByText("✓ 1 done").map((line) => line.parentElement?.style.paddingLeft)).toEqual(["2ch", "0ch"]);
  });

  it("u8_a_meta_agent_never_folds_even_when_done", async () => {
    await mountRail([metaAgent("lead", "done", "d", { status: { kind: "done", label: "d", since: NOW - 60 * MINUTE } })]);

    expect(rowNames()).toEqual(["lead"]);
  });

  it("u8_clicking_a_groups_triangle_collapses_it_to_its_most_urgent_descendants_glyph_and_ink", async () => {
    await mountRail([
      group("migrate"),
      agent("a", "working", "w", { parent: "migrate", order: 0 }),
      agent("b", "needs-you", "asks", { parent: "migrate", order: 1 }),
    ]);

    fireEvent.click(glyphOf("migrate"));

    expect([rowNames(), glyphOf("migrate").textContent, glyphOf("migrate").className]).toEqual([
      ["migrate"],
      "●",
      "glyph ink",
    ]);
  });

  it("u8_a_collapsed_group_shows_a_light_child_count", async () => {
    await mountRail([
      group("migrate"),
      agent("a", "working", "w", { parent: "migrate", order: 0 }),
      agent("b", "idle", "i", { parent: "migrate", order: 1 }),
    ]);

    fireEvent.click(glyphOf("migrate"));

    expect(rowOf("migrate").querySelector(".light")?.textContent).toBe("2");
  });

  it("u8_another_click_expands_a_collapsed_group", async () => {
    await mountRail([group("migrate"), agent("a", "working", "w", { parent: "migrate" })]);
    fireEvent.click(glyphOf("migrate"));

    fireEvent.click(glyphOf("migrate"));

    expect([rowNames(), glyphOf("migrate").textContent]).toEqual([["migrate", "a"], "▾"]);
  });

  it.each<[Kind, string]>([
    ["error", "✕"],
    ["blocked", "⏸"],
    ["done", "✓"],
  ])("u8_a_collapsed_group_with_a_%s_agent_below_shows_%s", async (kind, mark) => {
    await mountRail([group("g"), agent("a", kind, "l", { parent: "g" })]);

    fireEvent.click(glyphOf("g"));

    expect(glyphOf("g").textContent).toBe(mark);
  });

  it("u8_collapsing_a_group_does_not_select_it", async () => {
    const { rail } = await mountRail([group("g")]);

    fireEvent.click(glyphOf("g"));

    expect(rail.selected()).toBeNull();
  });

  it("u8_the_child_count_counts_direct_children_not_descendants", async () => {
    await mountRail([
      group("g"),
      group("inner", { parent: "g" }),
      agent("deep", "working", "w", { parent: "inner" }),
    ]);

    fireEvent.click(glyphOf("g"));

    expect(rowOf("g").querySelector(".light")?.textContent).toBe("1");
  });

  it("u8_a_collapsed_group_shows_the_most_urgent_kind_among_all_descendants_not_the_last_child", async () => {
    await mountRail([
      group("outer"),
      group("inner", { parent: "outer", order: 0 }),
      agent("asks", "needs-you", "?", { parent: "inner", order: 0 }),
      agent("busy", "working", "w", { parent: "outer", order: 1 }),
    ]);

    fireEvent.click(glyphOf("outer"));

    expect([glyphOf("outer").textContent, glyphOf("outer").className]).toEqual(["●", "glyph ink"]);
  });

  it("u8_a_collapsed_group_with_no_agent_below_shows_a_right_triangle", async () => {
    await mountRail([group("g"), terminal("t", { parent: "g" })]);

    fireEvent.click(glyphOf("g"));

    expect(glyphOf("g").textContent).toBe("▸");
  });
});
