import { cleanup, fireEvent, screen, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { agent, group } from "../testing/nodes";
import { rowOf } from "../rail/railFixture";
import type { EmulatorFactory } from "../terminal/emulator";
import { mountKeys } from "./keysFixture";

afterEach(cleanup);

/** A terminal screen that is actually focusable, standing in for xterm's real helper textarea (as in `u31_rail_keyboard.test.tsx`). */
const focusableEmulator = (): EmulatorFactory => () => {
  const field = document.createElement("textarea");

  return {
    write: () => {},
    onInput: () => {},
    show: (host) => {
      host.replaceChildren(field);
      field.focus();

      return { cols: 80, rows: 24 };
    },
    fit: () => ({ cols: 80, rows: 24 }),
    focus: () => field.focus(),
    isAtBottom: () => true,
    onScroll: () => {},
    scrollToBottom: () => {},
    dispose: () => {},
  };
};

const press = (key: string, held: KeyboardEventInit = {}) =>
  fireEvent.keyDown(document.activeElement ?? document, { key, ...held });

describe("u41 rail by keyboard, the rest", () => {
  it("u41_cmd_1_focuses_the_rail", async () => {
    const { rail } = await mountKeys([agent("a", "idle", "x")], { pane: focusableEmulator() });
    rail.select("a");
    screen.getByRole("textbox").focus();

    press("1", { metaKey: true });

    expect(document.activeElement).toBe(rowOf("a"));
  });

  it("u41_cmd_2_focuses_the_pane", async () => {
    const { rail } = await mountKeys([agent("a", "idle", "x")], { pane: focusableEmulator() });
    rail.select("a");
    rowOf("a").focus();

    press("2", { metaKey: true });

    expect(document.activeElement).toBe(document.querySelector(".pane-screen textarea"));
  });

  it("u41_cmd_1_and_cmd_2_are_handled_by_the_webview_so_the_browser_never_sees_them", async () => {
    await mountKeys([agent("a", "idle", "x")]);

    const handled = [
      fireEvent.keyDown(document, { key: "1", metaKey: true }),
      fireEvent.keyDown(document, { key: "2", metaKey: true }),
    ];

    const ignored = fireEvent.keyDown(document, { key: "1" });

    expect([handled, ignored]).toEqual([[false, false], true]);
  });

  it.each<[string, KeyboardEventInit]>([
    ["no modifier", {}],
    ["ctrl", { ctrlKey: true, metaKey: true }],
    ["alt", { altKey: true, metaKey: true }],
    ["cmd and shift", { metaKey: true, shiftKey: true }],
  ])("u41_a_cmd_1_with_another_modifier_does_nothing_%s", async (_, held) => {
    const { rail } = await mountKeys([agent("a", "idle", "x")], { pane: focusableEmulator() });
    rail.select("a");
    screen.getByRole("textbox").focus();
    const field = document.activeElement;

    press("1", held);

    expect(document.activeElement).toBe(field);
  });

  it("u41_left_on_an_expanded_group_collapses_it", async () => {
    await mountKeys([group("g"), agent("child", "idle", "x", { parent: "g" })]);
    rowOf("g").focus();

    press("ArrowLeft");

    expect(rowOf("g").getAttribute("aria-expanded")).toBe("false");
  });

  it("u41_left_on_a_collapsed_group_selects_its_parent", async () => {
    const { rail } = await mountKeys([group("outer"), group("inner", { parent: "outer" })]);
    fireEvent.click(within(rowOf("inner")).getByRole("button", { name: "collapse" }));
    rowOf("inner").focus();

    press("ArrowLeft");

    expect(rail.selected()).toBe("outer");
    expect(document.activeElement).toBe(rowOf("outer"));
  });

  it("u41_left_on_a_leaf_selects_its_parent", async () => {
    const { rail } = await mountKeys([group("g"), agent("child", "idle", "x", { parent: "g" })]);
    rowOf("child").focus();

    press("ArrowLeft");

    expect(rail.selected()).toBe("g");
    expect(document.activeElement).toBe(rowOf("g"));
  });

  it("u41_left_on_a_top_level_leaf_does_nothing", async () => {
    const { rail } = await mountKeys([agent("a", "idle", "x")]);
    rowOf("a").focus();

    press("ArrowLeft");

    expect(rail.selected()).toBeNull();
    expect(document.activeElement).toBe(rowOf("a"));
  });

  it("u41_right_on_a_collapsed_group_expands_it", async () => {
    await mountKeys([group("g"), agent("child", "idle", "x", { parent: "g" })]);
    fireEvent.click(screen.getByRole("button", { name: "collapse" }));
    rowOf("g").focus();

    press("ArrowRight");

    expect(rowOf("g").getAttribute("aria-expanded")).toBe("true");
    expect(screen.getAllByRole("treeitem")).toHaveLength(2);
  });

  it("u41_right_on_an_expanded_group_selects_its_first_child", async () => {
    const { rail } = await mountKeys([
      group("g"),
      agent("b", "idle", "x", { parent: "g", order: 1 }),
      agent("a", "idle", "x", { parent: "g", order: 0 }),
    ]);

    rowOf("g").focus();

    press("ArrowRight");

    expect(rail.selected()).toBe("a");
    expect(document.activeElement).toBe(rowOf("a"));
  });

  it("u41_right_on_a_leaf_with_no_children_does_nothing", async () => {
    const { rail } = await mountKeys([agent("a", "idle", "x")]);
    rowOf("a").focus();

    press("ArrowRight");

    expect(rail.selected()).toBeNull();
    expect(document.activeElement).toBe(rowOf("a"));
  });

  it("u41_arrow_left_and_right_in_the_rail_prevent_the_default_action", async () => {
    await mountKeys([group("g")]);
    rowOf("g").focus();

    expect(press("ArrowLeft")).toBe(false);
    expect(press("ArrowRight")).toBe(false);
  });

  it("u41_arrow_left_in_the_pane_reaches_the_terminal", async () => {
    const { rail } = await mountKeys(
      [group("g"), agent("a", "idle", "x", { parent: "g" })],
      { pane: focusableEmulator() },
    );

    rail.select("a");
    screen.getByRole("textbox").focus();
    const field = document.activeElement;

    const notPrevented = press("ArrowLeft");

    expect(notPrevented).toBe(true);
    expect(rail.selected()).toBe("a");
    expect(document.activeElement).toBe(field);
  });

  it("u41_arrow_left_in_the_rename_field_does_not_cancel_it", async () => {
    await mountKeys([agent("a", "idle", "x")]);
    fireEvent.dblClick(screen.getByText("a"));
    const field = screen.getByLabelText("name");
    field.focus();

    const notPrevented = press("ArrowLeft");

    expect(notPrevented).toBe(true);
    expect(document.activeElement).toBe(field);
  });

  it("u41_f2_edits_the_selected_rows_name_even_when_a_different_row_has_focus", async () => {
    const { rail } = await mountKeys([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rail.select("a");
    rowOf("b").focus();

    press("F2");

    expect(screen.getByLabelText("name").closest("[role=treeitem]")?.getAttribute("data-id")).toBe("a");
  });

  it("u41_f2_does_nothing_with_no_selection", async () => {
    await mountKeys([agent("a", "idle", "x")]);
    rowOf("a").focus();

    press("F2");

    expect(screen.queryByLabelText("name")).toBeNull();
  });

  it("u41_f2_on_the_collapse_button_does_nothing", async () => {
    const { rail } = await mountKeys([group("g")]);
    rail.select("g");
    screen.getByRole("button", { name: "collapse" }).focus();

    press("F2");

    expect(screen.queryByLabelText("name")).toBeNull();
  });
});
