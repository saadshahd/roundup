import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Kind } from "@contracts/Kind";
import { mountRail, rowOf } from "./railFixture";
import { agent, group, metaAgent, MINUTE, NOW, terminal } from "../testing/nodes";

afterEach(cleanup);

const since = (id: string, kind: Kind, minutesAgo: number, over = {}) =>
  agent(id, kind, "x", { status: { kind, label: "x", since: NOW - minutesAgo * MINUTE }, ...over });

const jump = () => fireEvent.keyDown(document, { key: "j", metaKey: true });

describe("u30 jump", () => {
  it("u30_the_header_counts_the_agents_that_need_you", async () => {
    await mountRail([since("a", "needs-you", 1), since("b", "error", 1), since("c", "working", 1), terminal("t")]);

    expect(screen.getByText("2 need you")).toBeDefined();
  });

  it("u30_the_header_shows_nothing_at_zero", async () => {
    await mountRail([since("a", "working", 1)]);

    expect(screen.queryByText(/need you/)).toBeNull();
  });

  it("u30_cmd_j_selects_error_before_needs_you", async () => {
    const { rail } = await mountRail([since("ask", "needs-you", 30), since("boom", "error", 1)]);

    jump();

    expect(rail.selected()).toBe("boom");
  });

  it("u30_within_a_kind_the_oldest_status_comes_first", async () => {
    const { rail } = await mountRail([since("new", "needs-you", 1), since("old", "needs-you", 9)]);

    jump();

    expect(rail.selected()).toBe("old");
  });

  it("u30_a_second_jump_selects_the_next_and_wraps_after_the_last", async () => {
    const { rail } = await mountRail([since("a", "error", 5), since("b", "needs-you", 5)]);

    jump();
    jump();
    jump();

    expect(rail.selected()).toBe("a");
  });

  it("u30_a_meta_agent_that_needs_you_is_visited", async () => {
    const { rail } = await mountRail([metaAgent("lead", "needs-you", "x")]);

    jump();

    expect(rail.selected()).toBe("lead");
  });

  it("u30_with_none_cmd_j_changes_nothing", async () => {
    const { rail } = await mountRail([since("a", "working", 1), since("b", "working", 1)]);

    rail.select("b");
    jump();

    expect(rail.selected()).toBe("b");
  });

  it("u30_equal_kind_and_status_age_are_taken_in_the_daemons_order", async () => {
    const { rail } = await mountRail([since("first", "needs-you", 5), since("second", "needs-you", 5)]);

    jump();

    expect(rail.selected()).toBe("first");
  });

  const pressed = async (chord: KeyboardEventInit) => {
    const { rail } = await mountRail([since("a", "needs-you", 1)]);

    fireEvent.keyDown(document, chord);

    return rail.selected();
  };

  it("u30_a_plain_j_changes_nothing", async () => {
    expect(await pressed({ key: "j" })).toBeNull();
  });

  it("u30_ctrl_j_changes_nothing", async () => {
    expect(await pressed({ key: "j", ctrlKey: true })).toBeNull();
  });

  it("u30_cmd_shift_j_changes_nothing", async () => {
    expect(await pressed({ key: "J", metaKey: true, shiftKey: true })).toBeNull();
  });

  it("u30_a_terminal_is_never_visited", async () => {
    const { rail } = await mountRail([terminal("t"), since("a", "needs-you", 1)]);

    jump();

    expect(rail.selected()).toBe("a");
  });

  it("u30_clicking_the_header_text_jumps", async () => {
    const { rail } = await mountRail([since("a", "needs-you", 1)]);

    fireEvent.click(screen.getByText("1 need you"));

    expect(rail.selected()).toBe("a");
  });

  it("u30_the_row_scrolls_into_view", async () => {
    const scroll = vi.spyOn(Element.prototype, "scrollIntoView");
    await mountRail([since("a", "needs-you", 1)]);

    jump();

    await vi.waitFor(() => expect(scroll.mock.contexts).toEqual([rowOf("a")]));
  });

  it("u30_a_collapsed_group_expands_to_show_the_row", async () => {
    await mountRail([group("g"), since("a", "needs-you", 1, { parent: "g" })]);
    fireEvent.click(screen.getByText("▾"));

    jump();

    expect(rowOf("a")).toBeDefined();
  });

  it("u30_the_rail_does_not_reorder", async () => {
    await mountRail([since("first", "working", 1, { order: 0 }), since("last", "error", 1, { order: 1 })]);

    jump();

    expect(screen.getAllByRole("treeitem").map((row) => row.querySelector(".name")?.textContent)).toEqual(["first", "last"]);
  });
});
