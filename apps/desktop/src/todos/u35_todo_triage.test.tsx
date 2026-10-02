import { cleanup, fireEvent, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RpcError } from "../app/seam";
import { USER } from "../testing/nodes";
import { callsTo, mountTodos, rowOf, todo, todoEvent } from "./testHarness";

afterEach(cleanup);

const drawer = () => screen.getByRole("complementary", { name: "drawer" });

const blocked = () => todo(5, { title: "session store", blocked: true, blockers: [4, 6] });

const shelf = () => [blocked(), todo(4, { title: "migrate" }), todo(6, { title: "later" })];

describe("u35 Todo triage without the Drawer", () => {
  it("u35_hovering_an_open_row_reveals_complete_which_calls_todo_complete", async () => {
    const { app } = await mountTodos([todo(3)]);
    await screen.findByText(/todo 3/);

    fireEvent.mouseEnter(rowOf(3));
    fireEvent.click(screen.getByText("complete"));

    await waitFor(() => expect(callsTo(app, "todo.complete")).toEqual([{ method: "todo.complete", params: { id: 3 } }]));
  });

  it("u35_complete_is_hidden_again_once_the_row_is_no_longer_hovered", async () => {
    await mountTodos([todo(3)]);
    await screen.findByText(/todo 3/);
    fireEvent.mouseEnter(rowOf(3));
    await screen.findByText("complete");

    fireEvent.mouseLeave(rowOf(3));

    expect(screen.queryByText("complete")).toBeNull();
  });

  it("u35_focusing_an_open_row_reveals_complete", async () => {
    await mountTodos([todo(3)]);
    const row = await screen.findByRole("button", { name: /#3/ });

    fireEvent.focusIn(row);

    expect(screen.getByText("complete")).toBeTruthy();
  });

  it("u35_complete_is_hidden_again_once_focus_leaves_the_row", async () => {
    await mountTodos([todo(3)]);
    const row = await screen.findByRole("button", { name: /#3/ });
    fireEvent.focusIn(row);
    await screen.findByText("complete");

    fireEvent.focusOut(row);

    expect(screen.queryByText("complete")).toBeNull();
  });

  it("u35_complete_logs_no_read_touch_and_does_not_open_the_drawer", async () => {
    const { app, connected } = await mountTodos([todo(3)]);
    await screen.findByText(/todo 3/);
    fireEvent.mouseEnter(rowOf(3));

    fireEvent.click(screen.getByText("complete"));
    await waitFor(() => expect(callsTo(app, "todo.complete")).toHaveLength(1));

    expect([callsTo(app, "todo.get"), connected.drawer.content()]).toEqual([[], null]);
  });

  it("u35_a_blockers_id_opens_that_blockers_drawer_and_not_the_blocked_todos_own_drawer", async () => {
    await mountTodos(shelf());
    await screen.findByText(/^waits on/);

    fireEvent.click(screen.getByText("#4"));

    await within(drawer()).findByText("+ blocker");
    // #4 has no blockers of its own; a "waits on" section would mean #5's drawer opened instead.
    expect([
      within(drawer()).queryByText("waits on"),
      within(drawer()).getByText("blocks").nextElementSibling?.textContent,
    ]).toEqual([null, "⏸ #5  session store"]);
  });

  it("u35_a_blockers_id_click_opens_the_drawer_exactly_once", async () => {
    const { connected } = await mountTodos(shelf());
    await screen.findByText(/^waits on/);
    const opened: unknown[] = [];
    const open = connected.drawer.open.bind(connected.drawer);
    connected.drawer.open = (content) => {
      opened.push(content);
      open(content);
    };

    fireEvent.click(screen.getByText("#4"));

    expect(opened).toHaveLength(1);
  });

  it("u35_the_blockers_id_is_reachable_by_keyboard", async () => {
    await mountTodos(shelf());
    await screen.findByText(/^waits on/);

    expect(screen.getByText("#4").closest("button")).not.toBeNull();
  });

  it("u35_a_failed_complete_shows_its_message_in_place_of_the_second_line", async () => {
    const { app } = await mountTodos(shelf());
    await screen.findByText(/^waits on/);
    app.handlers["todo.complete"] = () => Promise.reject(new RpcError(-32000, "cannot complete"));

    fireEvent.mouseEnter(rowOf(5));
    fireEvent.click(screen.getByText("complete"));

    expect((await screen.findByText(/cannot complete/)).textContent).toBe("✕ cannot complete");
    expect(screen.queryByText(/^waits on/)).toBeNull();
  });

  it("u35_a_failed_complete_still_calls_todo_complete_with_the_rows_id", async () => {
    const { app } = await mountTodos(shelf());
    await screen.findByText(/^waits on/);
    app.handlers["todo.complete"] = () => Promise.reject(new RpcError(-32000, "cannot complete"));

    fireEvent.mouseEnter(rowOf(5));
    fireEvent.click(screen.getByText("complete"));
    await screen.findByText(/cannot complete/);

    expect(callsTo(app, "todo.complete")).toEqual([{ method: "todo.complete", params: { id: 5 } }]);
  });

  it("u35_a_failed_completes_message_clears_on_the_next_click", async () => {
    const { app } = await mountTodos(shelf());
    await screen.findByText(/^waits on/);
    app.handlers["todo.complete"] = () => Promise.reject(new RpcError(-32000, "cannot complete"));
    fireEvent.mouseEnter(rowOf(5));
    fireEvent.click(screen.getByText("complete"));
    await screen.findByText(/cannot complete/);
    app.handlers["todo.complete"] = () => new Promise(() => {});

    fireEvent.click(screen.getByText("complete"));

    expect(screen.queryByText(/cannot complete/)).toBeNull();
  });

  it("u35_an_open_rows_text_at_rest_never_includes_complete", async () => {
    await mountTodos([todo(3)]);
    await screen.findByText(/todo 3/);

    expect(rowOf(3).textContent).toBe("· #3 todo 3");
    expect(screen.queryByText("complete")).toBeNull();
  });

  it("u35_a_successful_complete_leaves_the_row_unchanged_until_todo_updated_arrives", async () => {
    const { app } = await mountTodos([todo(3)]);
    await screen.findByText(/todo 3/);

    fireEvent.mouseEnter(rowOf(3));
    fireEvent.click(screen.getByText("complete"));
    await waitFor(() => expect(callsTo(app, "todo.complete")).toHaveLength(1));

    expect([callsTo(app, "todo.list"), screen.getByRole("button", { name: /#3/ }).textContent]).toEqual([
      [{ method: "todo.list", params: null }],
      "· #3 todo 3",
    ]);
  });

  it("u35_todo_updated_after_a_successful_complete_refetches_the_list_and_removes_the_row", async () => {
    const { app, store } = await mountTodos([todo(3)]);
    await screen.findByText(/todo 3/);
    fireEvent.mouseEnter(rowOf(3));
    fireEvent.click(screen.getByText("complete"));
    await waitFor(() => expect(callsTo(app, "todo.complete")).toHaveLength(1));

    store.todos = [todo(3, { done: true })];
    app.emit(todoEvent("todo.updated", todo(3, { done: true })));

    await waitFor(() => expect(screen.queryByRole("button", { name: /#3/ })).toBeNull());
    expect(callsTo(app, "todo.list")).toHaveLength(2);
  });

  it("u35_a_click_anywhere_in_the_shelf_clears_another_rows_failure", async () => {
    const { app } = await mountTodos(shelf());
    await screen.findByText(/^waits on/);
    app.handlers["todo.complete"] = () => Promise.reject(new RpcError(-32000, "cannot complete"));
    fireEvent.mouseEnter(rowOf(5));
    fireEvent.click(screen.getByText("complete"));
    await screen.findByText(/cannot complete/);

    fireEvent.click(rowOf(4));

    expect(screen.queryByText(/cannot complete/)).toBeNull();
  });

  it("u35_tabbing_from_the_row_to_complete_keeps_keyboard_focus_on_it", async () => {
    const { app } = await mountTodos([todo(3)]);
    const row = await screen.findByRole("button", { name: /#3/ });

    row.focus();
    const completeButton = screen.getByText("complete").closest("button");

    if (!completeButton) throw new Error("complete is not a button");

    completeButton.focus();

    expect([document.activeElement, completeButton.isConnected]).toEqual([completeButton, true]);

    fireEvent.click(completeButton);

    await waitFor(() => expect(callsTo(app, "todo.complete")).toEqual([{ method: "todo.complete", params: { id: 3 } }]));
  });

  it("u35_a_blocked_rows_second_line_changes_only_once_todo_unblocked_refetches_the_list", async () => {
    const { app, store } = await mountTodos([todo(5, { title: "session store", blocked: true, blockers: [4] }), todo(4, { title: "migrate" })]);
    await screen.findByText(/^waits on/);

    fireEvent.mouseEnter(rowOf(4));
    fireEvent.click(screen.getByText("complete"));
    await waitFor(() => expect(callsTo(app, "todo.complete")).toEqual([{ method: "todo.complete", params: { id: 4 } }]));
    store.todos = [todo(5, { title: "session store" }), todo(4, { title: "migrate", done: true })];

    // No todo.updated for #5: unblocking only ever reaches the webview as todo.unblocked (T3).
    expect(screen.getByText(/^waits on/).textContent).toBe("waits on #4");

    app.emit({ actor: USER, name: "todo.unblocked", data: { id: 5 } });

    await waitFor(() => expect(screen.queryByText(/^waits on/)).toBeNull());
  });

  it("u35_a_rows_shelf_click_listener_is_removed_once_the_shelf_unmounts", async () => {
    const addSpy = vi.spyOn(document, "addEventListener");
    const removeSpy = vi.spyOn(document, "removeEventListener");
    const { unmount } = await mountTodos([todo(3)]);
    await screen.findByText(/todo 3/);
    const added = addSpy.mock.calls.filter(([type]) => type === "click").length;

    unmount();

    const removed = removeSpy.mock.calls.filter(([type]) => type === "click").length;
    expect([added > 0, removed]).toEqual([true, added]);
    addSpy.mockRestore();
    removeSpy.mockRestore();
  });
});
