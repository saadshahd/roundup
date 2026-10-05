import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Kind } from "@contracts/Kind";
import chipStyles from "./attentionChip.styles.css?inline";
import { mountRail, rowOf } from "./railFixture";
import { agent, room, door, MINUTE, NOW, terminal } from "../testing/nodes";

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
    const { rail } = await mountRail([door("lead", "needs-you", "x")]);

    jump();

    expect(rail.selected()).toBe("lead");
  });

  it("u30_with_none_cmd_j_changes_nothing", async () => {
    const { rail } = await mountRail([since("a", "working", 1), since("b", "working", 1)]);

    rail.select("b");
    jump();

    expect(rail.selected()).toBe("b");
  });

  it("u30_with_none_selected_and_none_needing_you_cmd_j_does_not_throw", async () => {
    const onError = vi.fn();
    window.addEventListener("error", onError);

    const { rail } = await mountRail([since("a", "working", 1)]);
    jump();

    window.removeEventListener("error", onError);

    expect(onError).not.toHaveBeenCalled();
    expect(rail.selected()).toBeNull();
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
    await mountRail([room("g"), since("a", "needs-you", 1, { parent: "g" })]);
    fireEvent.click(screen.getByRole("button", { name: "collapse" }));

    jump();

    expect(rowOf("a")).toBeDefined();
  });

  it("u30_the_rail_does_not_reorder", async () => {
    await mountRail([since("first", "working", 1, { order: 0 }), since("last", "error", 1, { order: 1 })]);

    jump();

    expect(screen.getAllByRole("treeitem").map((row) => row.querySelector(".name")?.textContent)).toEqual(["first", "last"]);
  });
});

describe("u45 cmd+j follows u32's chord rules", () => {
  const pressed = async (chord: KeyboardEventInit) => {
    const { rail } = await mountRail([since("a", "needs-you", 1)]);

    fireEvent.keyDown(document, chord);

    return rail.selected();
  };

  it("u45_caps_lock_does_not_change_cmd_j", async () => {
    expect(await pressed({ key: "J", metaKey: true })).toBe("a");
  });

  it("u45_ctrl_cmd_j_changes_nothing", async () => {
    expect(await pressed({ key: "j", metaKey: true, ctrlKey: true })).toBeNull();
  });

  it("u45_alt_cmd_j_changes_nothing", async () => {
    expect(await pressed({ key: "j", metaKey: true, altKey: true })).toBeNull();
  });

  it("u45_cmd_with_another_key_changes_nothing", async () => {
    expect(await pressed({ key: "k", metaKey: true })).toBeNull();
  });

  it("u45_cmd_j_is_handled_by_the_webview_so_the_browser_never_sees_it", async () => {
    await mountRail([since("a", "needs-you", 1)]);

    const handled = fireEvent.keyDown(document, { key: "j", metaKey: true });
    const ignored = fireEvent.keyDown(document, { key: "j" });
    const ctrlCmd = fireEvent.keyDown(document, { key: "j", metaKey: true, ctrlKey: true });
    const altCmd = fireEvent.keyDown(document, { key: "j", metaKey: true, altKey: true });
    const shiftCmd = fireEvent.keyDown(document, { key: "J", metaKey: true, shiftKey: true });
    const cmdOther = fireEvent.keyDown(document, { key: "k", metaKey: true });

    expect([handled, ignored, ctrlCmd, altCmd, shiftCmd, cmdOther]).toEqual([false, true, true, true, true, true]);
  });
});

describe("u48 the attention chip looks like the control it is", () => {
  it("u48_the_chip_is_ink_weight", async () => {
    await mountRail([since("a", "needs-you", 1)]);

    expect(screen.getByText("1 need you").classList.contains("ink")).toBe(true);
  });

  it("u48_the_chip_names_its_chord_in_the_title", async () => {
    await mountRail([since("a", "needs-you", 1)]);

    expect(screen.getByTitle("⌘J").textContent).toBe("1 need you");
  });

  it("u48_the_chip_has_a_hover_rule", () => {
    expect(chipStyles).toMatch(/\.attention-chip:hover\s*\{[^}]*text-decoration:\s*underline/);
  });

  it("u48_the_rendered_chip_carries_the_class_the_hover_rule_targets", async () => {
    await mountRail([since("a", "needs-you", 1)]);

    const loaded = Array.from(document.styleSheets).some((sheet) =>
      Array.from(sheet.cssRules).some((rule) => rule.cssText.includes(".attention-chip:hover")),
    );

    expect([loaded, screen.getByText("1 need you").classList.contains("attention-chip")]).toEqual([true, true]);
  });

  it("u48_it_still_shows_nothing_at_zero", async () => {
    await mountRail([since("a", "working", 1)]);

    expect(screen.queryByText(/need you/)).toBeNull();
  });

  it("u48_it_still_jumps_on_click", async () => {
    const { rail } = await mountRail([since("a", "needs-you", 1)]);

    fireEvent.click(screen.getByText("1 need you"));

    expect(rail.selected()).toBe("a");
  });

  it("u48_after_daemon_exited_the_chip_stays", async () => {
    const [exit] = createSignal({ code: 1 });
    await mountRail([since("a", "needs-you", 1)], [], exit);

    expect(screen.getByText("1 need you")).toBeDefined();
  });

  it("u48_after_daemon_exited_cmd_j_still_selects", async () => {
    const [exit] = createSignal({ code: 1 });
    const { rail } = await mountRail([since("a", "needs-you", 1)], [], exit);

    jump();

    expect(rail.selected()).toBe("a");
  });
});
