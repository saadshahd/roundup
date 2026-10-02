import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { agent, event, group, MINUTE, NOW, terminal } from "../testing/nodes";
import type { EmulatorFactory } from "../terminal/emulator";
import styles from "./styles.css?inline";
import { mountRail, rowOf, tabbableRows } from "./railFixture";

afterEach(cleanup);

const press = (key: string) => fireEvent.keyDown(document.activeElement ?? document, { key });

/** A terminal screen that is actually focusable, standing in for xterm's real helper textarea. */
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
    dispose: () => {},
  };
};

describe("u31 rail keyboard", () => {
  it("u31_arrow_down_in_the_rename_field_does_not_cancel_the_rename", async () => {
    await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    fireEvent.dblClick(screen.getByText("a"));
    const field = screen.getByLabelText("name");
    field.focus();

    fireEvent.keyDown(field, { key: "ArrowDown" });

    expect(document.activeElement).toBe(field);
  });

  it("u31_enter_on_the_collapse_button_does_not_select_the_row", async () => {
    const { rail } = await mountRail([group("g")]);
    const button = screen.getByRole("button", { name: "collapse" });
    button.focus();

    fireEvent.keyDown(button, { key: "Enter" });

    expect(rail.selected()).toBeNull();
  });

  it("u31_down_moves_focus_to_the_next_row", async () => {
    await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rowOf("a").focus();

    press("ArrowDown");

    expect(document.activeElement).toBe(rowOf("b"));
  });

  it("u31_up_moves_focus_to_the_previous_row", async () => {
    const { rail } = await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x"), agent("c", "idle", "x")]);
    rail.select("b");
    rowOf("c").focus();

    press("ArrowUp");

    expect(document.activeElement).toBe(rowOf("b"));
  });

  it("u31_down_does_not_wrap_past_the_last_row", async () => {
    const { rail } = await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x"), agent("c", "idle", "x")]);
    rail.select("a");
    rowOf("c").focus();

    press("ArrowDown");

    expect(document.activeElement).toBe(rowOf("c"));
  });

  it("u31_down_after_that_enter_moves_from_the_row_that_has_focus", async () => {
    const { rail } = await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x"), agent("c", "idle", "x")]);
    rail.select("a");
    rowOf("b").focus();
    press("Enter");

    press("ArrowDown");

    expect(document.activeElement).toBe(rowOf("c"));
    expect(rail.selected()).toBe("b");
  });

  it("u31_arrow_down_moves_from_a_row_focused_directly_even_when_another_row_is_tabbable", async () => {
    await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x"), agent("c", "idle", "x")]);
    rowOf("b").focus();

    press("ArrowDown");

    expect(document.activeElement).toBe(rowOf("c"));
  });

  it("u31_up_does_not_wrap_past_the_first_row", async () => {
    await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rowOf("a").focus();

    press("ArrowUp");

    expect(document.activeElement).toBe(rowOf("a"));
  });

  it("u31_a_collapsed_groups_children_are_skipped", async () => {
    await mountRail([group("g"), agent("hidden", "idle", "x", { parent: "g" }), agent("after", "idle", "x")]);
    fireEvent.click(screen.getByText("▾"));
    rowOf("g").focus();

    press("ArrowDown");

    expect(document.activeElement).toBe(rowOf("after"));
  });

  it("u31_the_focused_row_has_a_visible_focus_ring", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;

    try {
      await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
      rowOf("a").focus();

      press("ArrowDown");

      expect(getComputedStyle(rowOf("b")).outlineStyle).toBe("solid");
    } finally {
      sheet.remove();
    }
  });

  it("u31_enter_selects_the_focused_row_as_a_click_would", async () => {
    const { rail } = await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rowOf("a").focus();
    press("ArrowDown");

    press("Enter");

    expect(rail.selected()).toBe("b");
  });

  it("u31_enter_calls_no_method_of_its_own", async () => {
    const { app, rail } = await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rowOf("a").focus();
    press("ArrowDown");
    const before = app.calls.length;

    press("Enter");

    expect(rail.selected()).toBe("b");
    expect(app.calls.length).toBe(before);
  });

  it("u31_exactly_one_row_is_tabbable", async () => {
    await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x"), agent("c", "idle", "x")]);

    expect(tabbableRows()).toHaveLength(1);
  });

  it("u31_with_nothing_selected_the_first_row_is_tabbable", async () => {
    await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);

    const tabbable = tabbableRows();

    expect(tabbable).toEqual([rowOf("a")]);
  });

  it("u31_moving_focus_with_the_arrow_keys_does_not_move_the_roving_tabindex", async () => {
    await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rowOf("a").focus();

    press("ArrowDown");

    expect(document.activeElement).toBe(rowOf("b"));
    expect(rowOf("a").tabIndex).toBe(0);
    expect(rowOf("b").tabIndex).toBe(-1);
  });

  it("u31_selecting_a_row_moves_the_roving_tabindex_to_it", async () => {
    const { rail } = await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    expect(tabbableRows()).toEqual([rowOf("a")]);

    rail.select("b");

    expect(tabbableRows()).toEqual([rowOf("b")]);
  });

  it("u31_deselecting_returns_the_roving_tabindex_to_the_first_row", async () => {
    const { rail } = await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rail.select("b");

    rail.select(null);

    expect(tabbableRows()).toEqual([rowOf("a")]);
  });

  it("u31_collapsing_the_selected_rows_group_moves_the_roving_tabindex_to_the_first_row", async () => {
    const { rail } = await mountRail([
      group("g"),
      agent("child", "idle", "x", { parent: "g" }),
      agent("after", "idle", "x"),
    ]);

    rail.select("child");

    fireEvent.click(screen.getByRole("button", { name: "collapse" }));

    expect(screen.getAllByRole("treeitem")).toHaveLength(2);
    expect(tabbableRows()).toEqual([rowOf("g")]);
  });

  it("u31_removing_the_selected_row_moves_the_roving_tabindex_to_the_first_row", async () => {
    const mounted = await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    mounted.rail.select("b");

    mounted.app.handlers["rail.tree"] = () => [agent("a", "idle", "x")];
    mounted.app.emit(event({ name: "rail.changed" }));
    await mounted.rail.settled();

    expect(tabbableRows()).toEqual([rowOf("a")]);
  });

  it("u31_folding_the_selected_row_moves_the_roving_tabindex_to_the_first_row", async () => {
    const { rail } = await mountRail([
      agent("old", "done", "finished", { status: { kind: "done", label: "finished", since: NOW - 10 * MINUTE }, order: 0 }),
      agent("after", "idle", "x", { order: 1 }),
    ]);

    rail.select("old");

    expect(tabbableRows()).toEqual([rowOf("after")]);
  });

  it("u31_typing_j_into_the_pane_never_reaches_the_rail", async () => {
    const { rail } = await mountRail(
      [agent("a", "idle", "x"), agent("b", "idle", "x")],
      [],
      undefined,
      undefined,
      { pane: focusableEmulator() },
    );

    rail.select("a");
    const field = document.activeElement;

    press("j");

    expect(rail.selected()).toBe("a");
    expect(document.activeElement).toBe(field);
  });

  it("u31_arrow_down_into_the_pane_never_moves_the_rails_focus", async () => {
    const { rail } = await mountRail(
      [agent("a", "idle", "x"), agent("b", "idle", "x")],
      [],
      undefined,
      undefined,
      { pane: focusableEmulator() },
    );

    rail.select("a");
    const field = document.activeElement;

    press("ArrowDown");

    expect(document.activeElement).toBe(field);
    expect(rowOf("a").tabIndex).toBe(0);
    expect(rowOf("b").tabIndex).toBe(-1);
  });

  it("u31_typing_j_into_a_drawer_never_reaches_the_rail", async () => {
    const { rail, connected } = await mountRail(
      [agent("a", "idle", "x"), agent("b", "idle", "x")],
      [],
      undefined,
      undefined,
      { drawer: true },
    );

    rail.select("a");
    connected.drawer.open(() => <input aria-label="field" />);
    const field = screen.getByLabelText("field");
    field.focus();

    press("j");

    expect(rail.selected()).toBe("a");
    expect(document.activeElement).toBe(field);
  });

  it("u31_arrow_down_into_a_drawer_never_moves_the_rails_focus", async () => {
    const { connected } = await mountRail(
      [agent("a", "idle", "x"), agent("b", "idle", "x")],
      [],
      undefined,
      undefined,
      { drawer: true },
    );

    connected.drawer.open(() => <input aria-label="field" />);
    const field = screen.getByLabelText("field");
    field.focus();

    press("ArrowDown");

    expect(document.activeElement).toBe(field);
    expect(rowOf("a").tabIndex).toBe(0);
    expect(rowOf("b").tabIndex).toBe(-1);
  });

  it("u31_the_rail_does_not_reorder", async () => {
    await mountRail([agent("first", "idle", "x", { order: 0 }), agent("last", "idle", "x", { order: 1 })]);
    rowOf("first").focus();

    press("ArrowDown");
    press("Enter");

    expect(screen.getAllByRole("treeitem").map((row) => row.querySelector(".name")?.textContent)).toEqual([
      "first",
      "last",
    ]);
  });

  it("u31_a_terminal_row_is_a_stop_for_arrow_navigation", async () => {
    await mountRail([agent("a", "idle", "x"), terminal("t")]);
    rowOf("a").focus();

    press("ArrowDown");

    expect(document.activeElement).toBe(rowOf("t"));
  });

  it("u31_a_plain_letter_key_inside_the_rail_does_nothing", async () => {
    const { rail } = await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rowOf("a").focus();

    press("j");

    expect(document.activeElement).toBe(rowOf("a"));
    expect(rail.selected()).toBeNull();
  });

  it("u31_enter_with_no_rows_does_nothing", async () => {
    const { rail } = await mountRail([]);

    fireEvent.keyDown(screen.getByRole("tree"), { key: "Enter" });

    expect(rail.selected()).toBeNull();
  });

  it("u31_the_selected_row_stays_tabbable_after_an_arrow_press_elsewhere", async () => {
    const { rail } = await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x"), agent("c", "idle", "x")]);
    fireEvent.click(rowOf("b"));

    press("ArrowDown");

    const tabbable = tabbableRows();
    expect(tabbable).toEqual([rowOf("b")]);
    expect(rail.selected()).toBe("b");
  });

  it("u31_reselecting_the_same_row_after_an_arrow_move_keeps_it_tabbable", async () => {
    await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    fireEvent.click(rowOf("a"));
    press("ArrowDown");
    fireEvent.click(rowOf("a"));

    const tabbable = tabbableRows();
    expect(tabbable).toEqual([rowOf("a")]);
  });

  it("u31_a_selection_made_elsewhere_takes_over_the_roving_tabindex", async () => {
    const { rail } = await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rowOf("a").focus();
    press("ArrowDown");

    rail.select("a");

    expect(rowOf("a").tabIndex).toBe(0);
    expect(rowOf("b").tabIndex).toBe(-1);
  });

  it("u31_a_row_removed_while_keyboard_focused_leaves_one_row_tabbable", async () => {
    const mounted = await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rowOf("a").focus();
    press("ArrowDown");

    mounted.app.handlers["rail.tree"] = () => [agent("a", "idle", "x")];
    mounted.app.emit(event({ name: "rail.changed" }));
    await mounted.rail.settled();

    const tabbable = tabbableRows();

    expect(tabbable).toEqual([rowOf("a")]);
  });

  it("u31_arrow_down_prevents_the_default_action", async () => {
    await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rowOf("a").focus();

    expect(press("ArrowDown")).toBe(false);
  });

  it("u31_enter_prevents_the_default_action", async () => {
    await mountRail([agent("a", "idle", "x")]);
    rowOf("a").focus();

    expect(press("Enter")).toBe(false);
  });

  it("u31_clicking_a_row_leaves_focus_on_its_terminal", async () => {
    await mountRail(
      [agent("a", "idle", "x"), agent("b", "idle", "x")],
      [],
      undefined,
      undefined,
      { pane: focusableEmulator() },
    );
    rowOf("a").focus();

    fireEvent.click(rowOf("b"));

    expect(document.activeElement?.tagName).toBe("TEXTAREA");
  });

  it("u31_enter_selecting_a_row_shows_its_terminal", async () => {
    await mountRail(
      [agent("a", "idle", "x"), agent("b", "idle", "x")],
      [],
      undefined,
      undefined,
      { pane: focusableEmulator() },
    );
    rowOf("a").focus();
    press("ArrowDown");

    press("Enter");

    expect(document.querySelector(".pane-title")?.textContent).toContain("b");
    expect(document.querySelector(".pane-screen textarea")).not.toBeNull();
  });
});
