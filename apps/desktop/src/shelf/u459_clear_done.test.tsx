import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, expect, it } from "vitest";
import { mountTodos, todo, todoCallsTo } from "../todos/testHarness";
import { useShelfScope } from "../todos/shelfScope";
import { SCOPE_ROW } from "../testing/nodes";
import { RpcError } from "../app/seam";
import { askToDelete, confirmDeletion, questionClosed, stubDialogLayout } from "./bulkQuestionHarness";

afterEach(cleanup);

stubDialogLayout();

it("u459_clear_done_targets_only_the_scoped_done_set", async () => {
  const { app, connected } = await mountTodos([todo(1, { done: true, home: SCOPE_ROW }), todo(2, { done: true, home: SCOPE_ROW }), todo(3, { done: true }), todo(4, { home: SCOPE_ROW })]);
  useShelfScope(connected).setShowAll(false);
  fireEvent.click(await screen.findByRole("button", { name: "Clear done" }));
  expect(await screen.findByText("Delete 2 done Todos?")).toBeTruthy();
  expect(todoCallsTo(app, "todo.delete")).toEqual([]);
});

it("u459_clear_done_confirms_once_and_cancels_without_calls", async () => {
  const { app } = await mountTodos([todo(1, { done: true }), todo(2, { done: true })]);
  const clear = await screen.findByRole("button", { name: "Clear done" });
  clear.focus();
  fireEvent.click(clear);
  await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("button", { name: "Keep" })));
  fireEvent.keyDown(screen.getByRole("button", { name: "Keep" }), { key: "Escape" });
  await waitFor(() => expect(document.activeElement).toBe(clear));
  expect(todoCallsTo(app, "todo.delete")).toEqual([]);
  expect(screen.queryByRole("button", { name: /#1 / })).toBeNull();
});

it("u459_clear_done_success_removes_only_done_rows", async () => {
  const { app } = await mountTodos([todo(1, { done: true }), todo(2, { done: true }), todo(3)]);
  fireEvent.click(await screen.findByRole("button", { name: "Clear done" }));
  await confirmDeletion();
  await waitFor(() => expect(screen.queryByRole("button", { name: "Clear done" })).toBeNull());
  await questionClosed();
  expect(todoCallsTo(app, "todo.delete").map((call) => call.params)).toEqual([{ id: 1 }, { id: 2 }]);
  expect(screen.getByRole("button", { name: /#3 / })).toBeTruthy();
});

it("u459_failed_done_rows_are_visible_and_selected", async () => {
  const { app, store } = await mountTodos([todo(1, { done: true }), todo(2, { done: true })]);
  app.handlers["todo.delete"] = (params) => {
    if (params.id === 2) throw new RpcError(-32001, "denied");
    store.todos = store.todos.filter((row) => row.id !== params.id);

    return null;
  };

  fireEvent.click(await screen.findByRole("button", { name: "Clear done" }));
  await confirmDeletion();
  await screen.findByText("todo 2 #2: denied");
  await questionClosed();
  expect(screen.getByRole("button", { name: /#2 / }).getAttribute("aria-pressed")).toBe("true");
  expect(screen.getByText("1 selected")).toBeTruthy();
});
