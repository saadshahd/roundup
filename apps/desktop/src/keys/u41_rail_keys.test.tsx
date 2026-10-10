import { cleanup, fireEvent, screen, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { agent, workstream, door, MINUTE, NOW } from "../testing/nodes";
import { rowOf } from "../rail/railFixture";
import type { EmulatorFactory } from "../terminal/emulator";
import { mountKeys } from "./keysFixture";

afterEach(cleanup);

/** A terminal screen that is actually focusable, standing in for xterm's real helper textarea (as in `u31_rail_keyboard.test.tsx`). */
const focusableEmulator = (): EmulatorFactory => () => {
  const field = document.createElement("textarea");

  return {
    write: () => {},
    setSize: () => {},
    setFontSize: () => {},
    reset: () => {},
    selection: () => "",
    paste: () => {},
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

/** An emulator that marks its field with the Terminal `id` it was created for, so a test can tell which Terminal the pane shows. */
const terminalMarkerEmulator = (): EmulatorFactory => (id: string) => {
  const field = document.createElement("textarea");

  field.dataset.terminal = id;

  return {
    write: () => {},
    setSize: () => {},
    setFontSize: () => {},
    reset: () => {},
    selection: () => "",
    paste: () => {},
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

  it("u41_cmd_1_focuses_the_selected_row_among_several", async () => {
    const { rail } = await mountKeys(
      [agent("a", "idle", "x", { order: 0 }), agent("b", "idle", "y", { order: 1 })],
      { pane: focusableEmulator() },
    );

    rail.select("b");
    screen.getByRole("textbox").focus();

    press("1", { metaKey: true });

    expect(document.activeElement).toBe(rowOf("b"));
  });

  it("u41_cmd_1_and_cmd_2_are_handled_by_the_webview_so_the_browser_never_sees_them", async () => {
    await mountKeys([agent("a", "idle", "x")]);

    const handled = [
      fireEvent.keyDown(document, { key: "1", metaKey: true }),
      fireEvent.keyDown(document, { key: "2", metaKey: true }),
    ];

    expect(handled).toEqual([false, false]);
  });

  it("u41_a_plain_1_with_no_modifier_is_ignored", async () => {
    await mountKeys([agent("a", "idle", "x")]);

    expect(fireEvent.keyDown(document, { key: "1" })).toBe(true);
  });

  it("u41_a_cmd_chord_with_another_key_does_nothing", async () => {
    await mountKeys([agent("a", "idle", "x")], { pane: focusableEmulator() });
    rowOf("a").focus();

    const notPrevented = fireEvent.keyDown(document.activeElement ?? document, { key: "3", metaKey: true });

    expect(notPrevented).toBe(true);
    expect(document.activeElement).toBe(rowOf("a"));
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
    await mountKeys([workstream("g"), agent("child", "idle", "x", { parent: "g" })]);
    rowOf("g").focus();

    press("ArrowLeft");

    expect(rowOf("g").getAttribute("aria-expanded")).toBe("false");
  });

  it("u41_left_on_an_expanded_group_among_several_collapses_only_the_focused_one", async () => {
    await mountKeys([workstream("g1", { order: 0 }), workstream("g2", { order: 1 })]);
    rowOf("g2").focus();

    press("ArrowLeft");

    expect(rowOf("g2").getAttribute("aria-expanded")).toBe("false");
    expect(rowOf("g1").getAttribute("aria-expanded")).toBe("true");
  });

  it("u41_left_on_a_collapsed_group_selects_its_parent", async () => {
    const { rail } = await mountKeys([workstream("outer"), workstream("inner", { parent: "outer" })]);
    fireEvent.click(within(rowOf("inner")).getByRole("button", { name: "collapse" }));
    rowOf("inner").focus();

    press("ArrowLeft");

    expect(rail.selected()).toBe("outer");
    expect(document.activeElement).toBe(rowOf("outer"));
  });

  it("u41_left_on_a_leaf_selects_its_parent", async () => {
    const { rail } = await mountKeys([workstream("g"), agent("child", "idle", "x", { parent: "g" })]);
    rowOf("child").focus();

    press("ArrowLeft");

    expect(rail.selected()).toBe("g");
    expect(document.activeElement).toBe(rowOf("g"));
  });

  it("u41_left_at_depth_three_selects_the_immediate_parent_not_the_root", async () => {
    const { rail } = await mountKeys([
      workstream("outer"),
      workstream("inner", { parent: "outer" }),
      agent("leaf", "idle", "x", { parent: "inner" }),
    ]);

    rowOf("leaf").focus();

    press("ArrowLeft");

    expect(rail.selected()).toBe("inner");
    expect(document.activeElement).toBe(rowOf("inner"));
  });

  it("u41_left_on_a_top_level_leaf_does_nothing", async () => {
    const { rail } = await mountKeys([agent("a", "idle", "x")]);
    rowOf("a").focus();

    press("ArrowLeft");

    expect(rail.selected()).toBeNull();
    expect(document.activeElement).toBe(rowOf("a"));
  });

  it("u41_right_on_a_collapsed_group_expands_it", async () => {
    await mountKeys([workstream("g"), agent("child", "idle", "x", { parent: "g" })]);
    fireEvent.click(screen.getByRole("button", { name: "collapse" }));
    rowOf("g").focus();

    press("ArrowRight");

    expect(rowOf("g").getAttribute("aria-expanded")).toBe("true");
    expect(screen.getAllByRole("treeitem")).toHaveLength(2);
  });

  it("u41_right_on_a_collapsed_group_among_several_expands_only_the_focused_one", async () => {
    await mountKeys([workstream("g1", { order: 0 }), workstream("g2", { order: 1 })]);
    fireEvent.click(within(rowOf("g1")).getByRole("button", { name: "collapse" }));
    fireEvent.click(within(rowOf("g2")).getByRole("button", { name: "collapse" }));
    rowOf("g2").focus();

    press("ArrowRight");

    expect(rowOf("g2").getAttribute("aria-expanded")).toBe("true");
    expect(rowOf("g1").getAttribute("aria-expanded")).toBe("false");
  });

  it("u41_right_on_an_expanded_group_selects_its_first_child", async () => {
    const { rail } = await mountKeys([
      workstream("g"),
      agent("b", "idle", "x", { parent: "g", order: 1 }),
      agent("a", "idle", "x", { parent: "g", order: 0 }),
    ]);

    rowOf("g").focus();

    press("ArrowRight");

    expect(rail.selected()).toBe("a");
    expect(document.activeElement).toBe(rowOf("a"));
  });

  it("u41_right_selects_the_first_visible_child_skipping_a_row_folded_into_done", async () => {
    const { rail } = await mountKeys([
      workstream("g"),
      agent("old", "done", "finished", { parent: "g", order: 0, status: { kind: "done", label: "finished", since: NOW - 11 * MINUTE } }),
      agent("live", "idle", "x", { parent: "g", order: 1 }),
    ]);

    rowOf("g").focus();

    press("ArrowRight");

    expect(rail.selected()).toBe("live");
    expect(document.activeElement).toBe(rowOf("live"));
  });

  it("u41_right_on_an_expanded_door_selects_its_first_child", async () => {
    const { rail } = await mountKeys([door("m", "working", "x"), agent("a", "idle", "y", { parent: "m" })]);

    rowOf("m").focus();

    press("ArrowRight");

    expect(rail.selected()).toBe("a");
    expect(document.activeElement).toBe(rowOf("a"));
  });

  it("u41_left_on_an_expanded_door_collapses_its_workstream", async () => {
    const { rail } = await mountKeys([
      workstream("g"),
      door("m", "working", "x", { parent: "g" }),
      agent("child", "idle", "y", { parent: "m" }),
    ]);

    rowOf("m").focus();

    press("ArrowLeft");

    expect(rail.collapsed().has("m")).toBe(true);
    expect(document.activeElement).toBe(rowOf("m"));
    expect(screen.getAllByRole("treeitem")).toHaveLength(2);
  });

  it("u41_right_on_a_leaf_with_no_children_does_nothing", async () => {
    const { rail } = await mountKeys([agent("a", "idle", "x")]);
    rowOf("a").focus();

    press("ArrowRight");

    expect(rail.selected()).toBeNull();
    expect(document.activeElement).toBe(rowOf("a"));
  });

  it("u41_right_on_a_leaf_followed_by_a_sibling_does_not_select_the_sibling", async () => {
    const { rail } = await mountKeys([
      agent("a", "idle", "x", { order: 0 }),
      agent("b", "idle", "y", { order: 1 }),
    ]);

    rowOf("a").focus();

    press("ArrowRight");

    expect(rail.selected()).toBeNull();
    expect(document.activeElement).toBe(rowOf("a"));
  });

  it("u41_arrow_left_and_right_in_the_rail_prevent_the_default_action", async () => {
    await mountKeys([workstream("g")]);
    rowOf("g").focus();

    expect(press("ArrowLeft")).toBe(false);
    expect(press("ArrowRight")).toBe(false);
  });

  it.each<[string, KeyboardEventInit]>([
    ["ctrl", { ctrlKey: true }],
    ["alt", { altKey: true }],
    ["shift", { shiftKey: true }],
    ["cmd", { metaKey: true }],
  ])("u41_arrow_left_with_a_modifier_does_nothing_%s", async (_, held) => {
    await mountKeys([workstream("g"), agent("child", "idle", "x", { parent: "g" })]);
    rowOf("g").focus();

    const notPrevented = press("ArrowLeft", held);

    expect(notPrevented).toBe(true);
    expect(rowOf("g").getAttribute("aria-expanded")).toBe("true");
  });

  it("u41_shift_f2_does_nothing", async () => {
    await mountKeys([agent("a", "idle", "x")]);
    rowOf("a").focus();

    press("F2", { shiftKey: true });

    expect(screen.queryByLabelText("name")).toBeNull();
  });

  it("u41_arrow_left_in_the_pane_reaches_the_terminal", async () => {
    const { rail } = await mountKeys(
      [workstream("g"), agent("a", "idle", "x", { parent: "g" })],
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

  it("u41_f2_prevents_the_default_action", async () => {
    const { rail } = await mountKeys([agent("a", "idle", "x")]);
    rail.select("a");
    rowOf("a").focus();

    expect(press("F2")).toBe(false);
  });

  it("u41_a_plain_key_other_than_f2_does_not_open_rename", async () => {
    const { rail } = await mountKeys([agent("a", "idle", "x")]);
    rail.select("a");
    rowOf("a").focus();

    press("j");

    expect(screen.queryByLabelText("name")).toBeNull();
  });

  it("u41_f2_on_the_collapse_button_does_nothing", async () => {
    const { rail } = await mountKeys([workstream("g")]);
    rail.select("g");
    screen.getByRole("button", { name: "collapse" }).focus();

    press("F2");

    expect(screen.queryByLabelText("name")).toBeNull();
  });

  it("u41_right_selecting_a_row_shows_its_terminal_exactly_as_a_click_does", async () => {
    await mountKeys(
      [workstream("g"), agent("child", "idle", "x", { parent: "g" })],
      { pane: terminalMarkerEmulator() },
    );
    rowOf("g").focus();

    press("ArrowRight");

    expect(document.querySelector<HTMLElement>(".pane-screen [data-terminal]")?.dataset.terminal).toBe("t-child");
  });

  it("u41_selecting_a_row_by_keyboard_calls_no_method_of_its_own", async () => {
    const { app } = await mountKeys([workstream("g"), agent("child", "idle", "x", { parent: "g" })]);
    rowOf("g").focus();
    const before = app.calls.length;

    press("ArrowRight");

    expect(app.calls.length).toBe(before);
  });

  it.each<[string, string]>([
    ["left", "ArrowLeft"],
    ["right", "ArrowRight"],
    ["f2", "F2"],
  ])("u41_%s_in_an_open_drawer_does_nothing", async (_, key) => {
    const { rail, connected } = await mountKeys(
      [workstream("g"), agent("child", "idle", "x", { parent: "g" })],
      { drawer: true },
    );

    rowOf("g").focus();
    connected.drawer.open(() => <input aria-label="field" />);
    screen.getByLabelText("field").focus();

    const notPrevented = press(key);

    expect(notPrevented).toBe(true);
    expect(rail.selected()).toBeNull();
    expect(screen.queryByLabelText("name")).toBeNull();
  });
});
