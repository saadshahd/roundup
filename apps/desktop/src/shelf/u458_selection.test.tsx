import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, expect, it } from "vitest";
import { mountTodos, todo, todoCallsTo } from "../todos/testHarness";
import { askToDelete, stubDialogLayout } from "./bulkQuestionHarness";

afterEach(cleanup);

stubDialogLayout();

it("u458_pointer_toggle_and_range_follow_visible_order", async () => {
  const { app, connected } = await mountTodos([todo(1), todo(2), todo(3), todo(4), todo(5)]);
  const first = await screen.findByRole("button", { name: /#1 / });
  fireEvent.click(first, { metaKey: true });
  expect(first.getAttribute("aria-pressed")).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: /#4 / }), { shiftKey: true });
  expect(screen.getByText("4 selected")).toBeTruthy();
  expect(connected.drawer.content()).toBeNull();
  expect(todoCallsTo(app, "todo.delete")).toEqual([]);
});

it("u458_keyboard_toggle_range_and_bounds", async () => {
  await mountTodos([todo(1), todo(2), todo(3)]);
  const first = await screen.findByRole("button", { name: /#1 / });
  first.focus();
  fireEvent.keyDown(first, { key: "Enter", metaKey: true });
  fireEvent.keyDown(first, { key: "Enter", metaKey: true, repeat: true });
  fireEvent.keyDown(first, { key: "ArrowDown", shiftKey: true });
  expect(document.activeElement).toBe(screen.getByRole("button", { name: /#2 / }));
  expect(screen.getByText("2 selected")).toBeTruthy();
});

it("u460_one_question_precedes_every_delete", async () => {
  const { app } = await mountTodos([todo(1), todo(2)]);
  fireEvent.click(await screen.findByRole("button", { name: /#1 / }), { metaKey: true });
  fireEvent.click(screen.getByRole("button", { name: /#2 / }), { metaKey: true });
  await askToDelete(2);
  expect(screen.getByText("Delete 2 Todos?")).toBeTruthy();
  expect(todoCallsTo(app, "todo.delete")).toEqual([]);
  fireEvent.click(screen.getByRole("button", { name: "Keep" }));
  expect(screen.getByText("2 selected")).toBeTruthy();
});

it("u458_plain_click_opens_the_drawer", async () => {
  const { connected } = await mountTodos([todo(1)]);
  const row = await screen.findByRole("button", { name: /#1 / });
  fireEvent.click(row, { metaKey: true });
  fireEvent.click(row);
  expect(row.getAttribute("aria-pressed")).toBe("false");
  expect(connected.drawer.content()).not.toBeNull();
});

it("u460_bar_counts_unique_items_and_cancel_clears_without_calls", async () => {
  const { app } = await mountTodos([todo(1)]);
  fireEvent.click(await screen.findByRole("button", { name: /#1 / }), { metaKey: true });
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(screen.queryByText("1 selected")).toBeNull();
  expect(todoCallsTo(app, "todo.delete")).toEqual([]);
});

it("u458_keys_leave_other_controls_and_reorder_unchanged", async () => {
  const { app } = await mountTodos([todo(1), todo(2)]);
  const first = await screen.findByRole("button", { name: /#1 / });

  for (const init of [{ altKey: true }, { ctrlKey: true }, { shiftKey: true }]) {
    fireEvent.keyDown(first, { key: "Enter", metaKey: true, ...init });
  }

  fireEvent.keyDown(screen.getByRole("checkbox", { name: "Complete todo 1" }), { key: "Enter", metaKey: true });
  expect(screen.queryByText(/selected/)).toBeNull();
  first.focus();
  fireEvent.keyDown(first, { key: "ArrowDown", altKey: true });
  expect(todoCallsTo(app, "todo.reorder")).toHaveLength(1);
});

it("u458_ranges_skip_folded_rows_and_keep_a_toggled_off_anchor", async () => {
  await mountTodos([todo(1), todo(2, { done: true }), todo(3), todo(4)]);
  const first = await screen.findByRole("button", { name: /#1 / });
  fireEvent.click(first, { metaKey: true });
  fireEvent.click(first, { metaKey: true });
  fireEvent.click(screen.getByRole("button", { name: /#4 / }), { metaKey: true, shiftKey: true });
  expect(screen.getByText("3 selected")).toBeTruthy();
  expect(screen.queryByRole("button", { name: /#2 / })).toBeNull();
  expect(first.getAttribute("aria-pressed")).toBe("true");
});

it("u458_arrows_move_focus_without_wrapping_or_changing_selection", async () => {
  await mountTodos([todo(1), todo(2)]);
  const first = await screen.findByRole("button", { name: /#1 / });
  const last = screen.getByRole("button", { name: /#2 / });
  first.focus();
  fireEvent.keyDown(first, { key: "ArrowUp", shiftKey: true });
  expect(document.activeElement).toBe(first);
  expect(screen.queryByText(/selected/)).toBeNull();
  fireEvent.keyDown(first, { key: "ArrowDown" });
  expect(document.activeElement).toBe(last);
  fireEvent.keyDown(last, { key: "ArrowDown" });
  expect(document.activeElement).toBe(last);
  expect(screen.queryByText(/selected/)).toBeNull();
});
