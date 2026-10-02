import { cleanup, fireEvent, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { ConnectedProjectContext, connectProject } from "../state/connectedProject";
import { createFakeApp } from "../testing/fakeApp";
import { agent, event, group, NOW, terminal } from "../testing/nodes";
import { Pane } from "../terminal/Pane";
import { Rail } from "./Rail";
import styles from "./styles.css?inline";
import { mountRail, rowOf } from "./railFixture";

afterEach(cleanup);

const press = (key: string) => fireEvent.keyDown(document.activeElement ?? document, { key });

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
    await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rowOf("b").focus();

    press("ArrowUp");

    expect(document.activeElement).toBe(rowOf("a"));
  });

  it("u31_down_does_not_wrap_past_the_last_row", async () => {
    await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rowOf("b").focus();

    press("ArrowDown");

    expect(document.activeElement).toBe(rowOf("b"));
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
    await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rowOf("a").focus();

    press("ArrowDown");

    const ring = getComputedStyle(rowOf("b")).outlineStyle;
    sheet.remove();

    expect(ring).toBe("solid");
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
    const { rail } = await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rail.select("b");

    const tabbable = screen.getAllByRole("treeitem").filter((row) => row.tabIndex === 0);

    expect(tabbable).toEqual([rowOf("b")]);
  });

  it("u31_with_nothing_selected_the_first_row_is_tabbable", async () => {
    await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);

    const tabbable = screen.getAllByRole("treeitem").filter((row) => row.tabIndex === 0);

    expect(tabbable).toEqual([rowOf("a")]);
  });

  it("u31_moving_focus_moves_the_roving_tabindex", async () => {
    await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rowOf("a").focus();

    press("ArrowDown");

    expect(rowOf("a").tabIndex).toBe(-1);
    expect(rowOf("b").tabIndex).toBe(0);
  });

  it("u31_typing_j_into_the_pane_never_reaches_the_rail", async () => {
    const { rail } = await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rail.select("a");
    const outside = document.body.appendChild(document.createElement("input"));
    outside.focus();

    fireEvent.keyDown(outside, { key: "j" });
    fireEvent.keyDown(outside, { key: "ArrowDown" });

    expect(rail.selected()).toBe("a");
    expect(document.activeElement).toBe(outside);
    expect(rowOf("b").tabIndex).toBe(-1);
    outside.remove();
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

    const tabbable = screen.getAllByRole("treeitem").filter((row) => row.tabIndex === 0);

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

  it("u31_clicking_an_unfocused_row_moves_focus_to_it", async () => {
    await mountRail([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rowOf("a").focus();

    fireEvent.click(rowOf("b"));

    expect(document.activeElement).toBe(rowOf("b"));
  });

  it("u31_enter_selecting_a_row_shows_its_terminal", async () => {
    const app = createFakeApp();

    const tree = [
      agent("a", "idle", "x", { terminal_id: null }),
      agent("b", "idle", "x", { terminal_id: null }),
    ];

    app.handlers["rail.tree"] = () => tree;
    const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, () => NOW);

    render(() => (
      <ConnectedProjectContext.Provider value={connected}>
        <Rail />
        <Pane />
      </ConnectedProjectContext.Provider>
    ));
    rowOf("a").focus();
    press("ArrowDown");

    press("Enter");

    expect(document.querySelector(".pane-title")?.textContent).toContain("b");
  });
});
