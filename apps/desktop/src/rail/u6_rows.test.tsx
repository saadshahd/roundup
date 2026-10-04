import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { exitedTerminal, glyphOf, mountRail, rowNames, rowOf } from "./railFixture";
import { agent, event, room, terminal } from "../testing/nodes";

afterEach(cleanup);

describe("u6 rows", () => {
  it("u6_each_node_is_one_row_in_order_indented_2_per_depth_under_its_parent", async () => {
    await mountRail([
      room("migrate", { order: 1 }),
      agent("rename-cols", "working", "writing", { parent: "migrate", order: 1 }),
      agent("backfill", "idle", "idle", { parent: "migrate", order: 0 }),
      agent("checkout", "working", "planning", { order: 0 }),
    ]);

    expect(rowNames()).toEqual(["checkout", "migrate", "backfill", "rename-cols"]);
    expect(screen.getAllByRole("treeitem").map((row) => row.style.paddingLeft)).toEqual(["0ch", "0ch", "2ch", "2ch"]);
  });

  it("u6_an_agent_shows_the_glyph_of_its_status_kind", async () => {
    await mountRail([agent("gateway", "needs-you", "asks")]);

    expect(glyphOf("gateway").getAttribute("aria-label")).toBe("needs-you");
  });

  it("u6_a_meta_agent_shows_its_status_glyph", async () => {
    await mountRail([room("checkout", { incarnation: "1", status: { kind: "idle", label: "idle", since: 0 } })]);

    expect(glyphOf("checkout").getAttribute("aria-label")).toBe("idle");
  });

  it("u6_a_plain_group_shows_a_down_triangle", async () => {
    await mountRail([room("migrate")]);

    expect(rowOf("migrate").querySelector("button[aria-label=collapse] svg.lucide-chevron-down")).not.toBeNull();
  });

  it("u6_a_running_terminal_shows_a_circle", async () => {
    await mountRail([terminal("dev")]);

    expect(glyphOf("dev").getAttribute("aria-label")).toBe("working");
  });

  it("u6_an_exited_terminal_shows_a_check", async () => {
    await mountRail([terminal("dev")], [exitedTerminal("dev", 0)]);

    expect(glyphOf("dev").getAttribute("aria-label")).toBe("done");
  });

  it("u6_a_terminal_that_exits_changes_its_glyph_without_moving", async () => {
    const { app, rail } = await mountRail([terminal("dev"), agent("docs", "working", "w")]);

    app.emit(event({ name: "terminal.exited", data: { id: "t-dev", code: 1 } }));
    await rail.settled();

    expect([glyphOf("dev").getAttribute("aria-label"), rowNames()]).toEqual(["done", ["dev", "docs"]]);
  });

  it("u6_the_selected_row_has_a_faint_band", async () => {
    const { rail } = await mountRail([agent("a", "working", "w"), agent("b", "working", "w", { order: 1 })]);

    rail.select("b");

    expect([rowOf("a").dataset.selected, rowOf("b").dataset.selected]).toEqual(["false", "true"]);
  });

  it("u6_clicking_a_row_selects_it", async () => {
    const { rail } = await mountRail([agent("a", "working", "w")]);

    fireEvent.click(rowOf("a"));

    expect(rail.selected()).toBe("a");
  });

  it("u6_rows_never_reorder_when_a_status_changes", async () => {
    const { app, rail } = await mountRail([
      agent("a", "working", "w", { order: 0 }),
      agent("b", "working", "w", { order: 1 }),
    ]);

    app.emit(event({ name: "agent.status", data: { incarnation: "1", id: "b", status: { kind: "error", label: "x", since: 2 } } }));
    await rail.settled();

    expect([rowNames(), glyphOf("b").getAttribute("aria-label")]).toEqual([["a", "b"], "error"]);
  });
});
