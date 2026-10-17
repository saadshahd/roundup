import { cleanup, fireEvent, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, expect, it } from "vitest";
import { mountTodos, todo, todoCallsTo } from "../todos/testHarness";
import { deferred } from "../pads/padsFixture";
import { RpcError } from "../app/seam";
import { USER } from "../testing/nodes";
import { useShelfScope } from "../todos/shelfScope";
import { stubDialogLayout, askToDelete, confirmDeletion, questionClosed, questionDialog } from "./bulkQuestionHarness";

afterEach(cleanup);

stubDialogLayout();

const pickTwo = async () => {
  fireEvent.click(await screen.findByRole("button", { name: /#1 / }), { metaKey: true });
  fireEvent.click(screen.getByRole("button", { name: /#2 / }), { metaKey: true });
};

it("u460_each_confirmed_target_is_called_once", async () => {
  const { app, store } = await mountTodos([todo(1), todo(2)]);
  const held = deferred<null>();
  app.handlers["todo.delete"] = async (params) => {
    if (params.id === 1) await held.promise;
    store.todos = store.todos.filter((row) => row.id !== params.id);

    return null;
  };

  await pickTwo();
  await askToDelete(2);
  await confirmDeletion();
  await confirmDeletion();
  expect(todoCallsTo(app, "todo.delete")).toHaveLength(1);
  held.resolve(null);
  await questionClosed();
  expect(todoCallsTo(app, "todo.delete").map((call) => call.params)).toEqual([{ id: 1 }, { id: 2 }]);
});

it("u460_partial_failure_names_and_selects_only_failed_rows", async () => {
  const { app, store } = await mountTodos([todo(1, { title: "same" }), todo(2, { title: "same" })]);
  app.handlers["todo.delete"] = (params) => {
    if (params.id === 2) throw new RpcError(-32001, "denied");
    store.todos = store.todos.filter((row) => row.id !== params.id);

    return null;
  };

  await pickTwo();
  await askToDelete(2);
  await confirmDeletion();
  await screen.findByText("same #2: denied");
  expect(screen.getByText("1 selected")).toBeTruthy();
  expect(screen.queryByRole("button", { name: /#1 / })).toBeNull();
  expect(screen.getByRole("button", { name: /#2 / }).getAttribute("aria-pressed")).toBe("true");
});

it("u460_lost_response_refetches_before_retry", async () => {
  const { app, store } = await mountTodos([todo(1), todo(2)]);
  app.handlers["todo.delete"] = (params) => {
    store.todos = store.todos.filter((row) => row.id !== params.id);
    throw new Error("response lost");
  };

  await pickTwo();
  await askToDelete(2);
  await confirmDeletion();
  await questionClosed();
  expect(todoCallsTo(app, "todo.list").length).toBeGreaterThan(1);
});

it("u460_cancel_keeps_selection_and_restores_focus", async () => {
  await mountTodos([todo(1), todo(2)]);
  await pickTwo();
  const button = screen.getByRole("button", { name: "Delete 2" });
  button.focus();
  await askToDelete(2);
  await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("button", { name: "Keep" })));
  fireEvent.click(screen.getByRole("button", { name: "Keep" }));
  await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("button", { name: "Delete 2" })));
  expect(screen.getByText("2 selected")).toBeTruthy();
});

it("u460_confirmation_never_expands_or_outlives_its_scope", async () => {
  const { app, store, connected } = await mountTodos([todo(1), todo(2)]);
  await pickTwo();
  await askToDelete(2);
  store.todos.push(todo(3));
  app.emit({ actor: USER, name: "todo.created", data: todo(3) });
  await screen.findByRole("button", { name: /#3 / });
  expect(screen.getByText("Delete 2 Todos?")).toBeTruthy();
  useShelfScope(connected).setShowAll(false);
  await questionClosed();
  expect(todoCallsTo(app, "todo.delete")).toEqual([]);
});

it("u460_backspace_is_local_and_repeat_safe", async () => {
  const { app } = await mountTodos([todo(1), todo(2)]);
  await pickTwo();
  const row = screen.getByRole("button", { name: /#1 / });
  row.focus();
  fireEvent.keyDown(row, { key: "Backspace", repeat: true });
  fireEvent.keyDown(row, { key: "Backspace", metaKey: true });
  fireEvent.keyDown(screen.getByRole("checkbox", { name: "Complete todo 1" }), { key: "Backspace" });
  expect(screen.queryByRole("button", { name: "Keep" })).toBeNull();
  fireEvent.keyDown(row, { key: "Backspace" });
  expect(await screen.findByText("Delete 2 Todos?")).toBeTruthy();
  expect(todoCallsTo(app, "todo.delete")).toEqual([]);
});

it("u460_retry_confirms_only_the_remaining_set", async () => {
  const { app, store } = await mountTodos([todo(1), todo(2)]);
  let failed = true;
  app.handlers["todo.delete"] = (params) => {
    if (params.id === 2 && failed) throw new RpcError(-32001, "denied");
    store.todos = store.todos.filter((row) => row.id !== params.id);

    return null;
  };

  await pickTwo();
  await askToDelete(2);
  await confirmDeletion();
  await screen.findByText("todo 2 #2: denied");
  failed = false;
  await askToDelete(1);
  expect(screen.getByText("Delete 1 Todos?")).toBeTruthy();
  await confirmDeletion();
  await questionClosed();
  expect(todoCallsTo(app, "todo.delete").map((call) => call.params)).toEqual([{ id: 1 }, { id: 2 }, { id: 2 }]);
});

it("u460_failed_readback_blocks_retry_until_a_fresh_list_answers", async () => {
  const { app, store } = await mountTodos([todo(1), todo(2)]);
  await pickTwo();
  let lost = true;
  app.handlers["todo.list"] = () => {
    if (lost) throw new Error("list unavailable");

    return store.todos;
  };

  app.handlers["todo.delete"] = () => { throw new Error("response lost"); };

  await askToDelete(2);
  await confirmDeletion();
  await screen.findByRole("button", { name: "Refresh" });
  expect(within(await questionDialog()).getByRole("button", { name: "Keep" }).hasAttribute("disabled")).toBe(true);
  lost = false;
  fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
  await screen.findByText("2 selected");
  expect(todoCallsTo(app, "todo.delete")).toHaveLength(2);
});

it("u460_readback_waits_for_a_newer_event_refetch_before_retry", async () => {
  const { app } = await mountTodos([todo(1), todo(2)]);
  const old = deferred<ReturnType<typeof todo>[]>();
  const fresh = deferred<ReturnType<typeof todo>[]>();
  let reads = 0;
  app.handlers["todo.list"] = () => ++reads === 1 ? old.promise : fresh.promise;
  app.handlers["todo.delete"] = () => { throw new Error("response lost"); };

  await pickTwo();
  await askToDelete(2);
  await confirmDeletion();
  await waitFor(() => expect(reads).toBe(1));
  app.emit({ actor: USER, name: "todo.updated", data: todo(2) });
  await waitFor(() => expect(reads).toBe(2));
  old.resolve([todo(1), todo(2)]);
  // Observe that no retry is offered while the newer list response is withheld.
  await new Promise((resolve) => setTimeout(resolve, 40));
  expect(within(await questionDialog()).getByRole("button", { name: "Keep" }).hasAttribute("disabled")).toBe(true);
  fresh.resolve([todo(2)]);
  await screen.findByText("1 selected");
  await questionClosed();
  expect(screen.getByRole("button", { name: /#2 / }).getAttribute("aria-pressed")).toBe("true");
});

it("u460_plain_click_after_failure_clears_the_bar_before_opening_the_drawer", async () => {
  const { app } = await mountTodos([todo(1), todo(2)]);
  app.handlers["todo.delete"] = () => { throw new Error("denied"); };

  await pickTwo();
  await askToDelete(2);
  await confirmDeletion();
  await screen.findByText("todo 1 #1: denied");
  fireEvent.click(screen.getByRole("button", { name: /#1 / }));
  expect(screen.queryByRole("group", { name: "Work items deletion" })).toBeNull();
  expect(screen.getByRole("complementary", { name: "Drawer" })).toBeTruthy();
});
