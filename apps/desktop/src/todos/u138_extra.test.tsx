import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { RpcError } from "../app/seam";
import { event } from "../testing/nodes";
import { mountTodos, todo, todoCallsTo, todoEvent } from "./testHarness";
import { rowAfter } from "./Todos";

afterEach(cleanup);

describe("u138 rowAfter", () => {
  it("u138_extra_row_after_picks_the_next_open_id", () => {
    expect(rowAfter(2, [1, 2, 3])).toBe(3);
  });

  it("u138_extra_row_after_falls_back_to_the_previous_open_id", () => {
    expect(rowAfter(3, [1, 2, 3])).toBe(2);
  });

  it("u138_extra_row_after_is_null_with_no_other_open_id", () => {
    expect(rowAfter(1, [1])).toBeNull();
  });
});

describe("u138 a failed complete schedules no later focus move", () => {
  it("u138_extra_a_failed_complete_does_not_move_focus_when_the_row_later_leaves_by_another_route", async () => {
    const mounted = await mountTodos([todo(1), todo(2)]);
    mounted.app.handlers["todo.complete"] = () => {
      throw new RpcError(-32000, "boom");
    };

    (await screen.findByRole("button", { name: /#1 / })).focus();
    const complete = await screen.findByRole("button", { name: "complete" });

    complete.focus();
    fireEvent.click(complete);
    await screen.findByText(/boom/);

    // #1 now leaves the open list by deletion, not by the keyboard-complete flow U138 governs; the failed
    // attempt above must not have left a focus move scheduled for whenever #1 next leaves the list.
    mounted.store.todos = mounted.store.todos.filter((candidate) => candidate.id !== 1);
    mounted.app.emit(event({ name: "todo.deleted", data: { id: 1 } }));

    await waitFor(() => expect(screen.queryByText(/#1 /)).toBeNull());
    await waitFor(() => expect(todoCallsTo(mounted.app, "todo.complete")).toHaveLength(1));
    expect(document.activeElement).toBe(document.body);
  });
});

describe("u138 focus already moved elsewhere is never reclaimed", () => {
  it("u138_extra_focus_moved_elsewhere_before_the_row_leaves_stays_there", async () => {
    const mounted = await mountTodos([todo(1), todo(2), todo(3)]);

    (await screen.findByRole("button", { name: /#2 / })).focus();
    fireEvent.click(await screen.findByRole("button", { name: "complete" }));
    await waitFor(() => expect(todoCallsTo(mounted.app, "todo.complete")).toHaveLength(1));

    fireEvent.click(screen.getByRole("button", { name: "+" }));
    const input = await screen.findByLabelText("new todo title");
    input.focus();

    mounted.app.emit(todoEvent("todo.updated", { ...mounted.store.todos.find((candidate) => candidate.id === 2)! }));

    await waitFor(() => expect(screen.queryByText(/#2 /)).toBeNull());
    expect(document.activeElement).toBe(input);
  });
});

describe("u138 focus stays put until the Todo actually leaves the open list", () => {
  it("u138_extra_focus_stays_on_complete_until_the_todo_leaves_the_open_list", async () => {
    const mounted = await mountTodos([todo(1), todo(2), todo(3)]);

    (await screen.findByRole("button", { name: /#2 / })).focus();
    const complete = await screen.findByRole("button", { name: "complete" });

    complete.focus();
    fireEvent.click(complete);
    await waitFor(() => expect(todoCallsTo(mounted.app, "todo.complete")).toHaveLength(1));

    // No `todo.updated` has reached the list yet, so #2 is still open; focus must not already have
    // jumped to its target before the row that held it is actually gone.
    expect(document.activeElement).toBe(complete);
  });
});

describe("u138 the target is fixed at press time, not recomputed after the wait", () => {
  it("u138_extra_the_target_is_the_row_after_the_pressed_one_at_press_time", async () => {
    const mounted = await mountTodos([todo(1), todo(2), todo(3)]);
    const original = mounted.app.handlers["todo.complete"]!;
    let release: (() => void) | undefined;

    mounted.app.handlers["todo.complete"] = (params) =>
      new Promise((resolve) => {
        release = () => resolve(original(params));
      });

    (await screen.findByRole("button", { name: /#2 / })).focus();
    fireEvent.click(await screen.findByRole("button", { name: "complete" }));
    await waitFor(() => expect(todoCallsTo(mounted.app, "todo.complete")).toHaveLength(1));

    // #3 leaves the open list by another route while #2's complete call is still pending; the target
    // for #2 must already be fixed at #3 from press time, not recomputed from the shrunken list later
    // (which would instead point at #1).
    mounted.store.todos = mounted.store.todos.filter((candidate) => candidate.id !== 3);
    mounted.app.emit(event({ name: "todo.deleted", data: { id: 3 } }));
    await waitFor(() => expect(screen.queryByText(/#3 /)).toBeNull());

    release?.();
    mounted.app.emit(todoEvent("todo.updated", { ...mounted.store.todos.find((candidate) => candidate.id === 2)! }));

    await waitFor(() => expect(screen.queryByText(/#2 /)).toBeNull());
    expect(document.activeElement).toBe(await screen.findByRole("button", { name: "+" }));
  });
});

describe("u138 two rows leaving together fall back when each named the other as its target", () => {
  it("u138_extra_both_rows_gone_focuses_the_plus_button_even_though_each_targeted_the_other", async () => {
    const mounted = await mountTodos([todo(1), todo(2)]);

    // Each row's `target` is the other, picked while both were still open; if the row-ref map kept
    // either entry past its row's unmount, the stale, disconnected button would win over the `+` fallback.
    (await screen.findByRole("button", { name: /#1 / })).focus();
    fireEvent.click(await screen.findByRole("button", { name: "complete" }));
    await waitFor(() => expect(todoCallsTo(mounted.app, "todo.complete")).toHaveLength(1));

    (await screen.findByRole("button", { name: /#2 / })).focus();
    fireEvent.click(await screen.findByRole("button", { name: "complete" }));
    await waitFor(() => expect(todoCallsTo(mounted.app, "todo.complete")).toHaveLength(2));

    mounted.app.emit(todoEvent("todo.updated", { ...mounted.store.todos.find((candidate) => candidate.id === 1)! }));

    await waitFor(() => expect(screen.queryByText(/#1 /)).toBeNull());
    await waitFor(() => expect(screen.queryByText(/#2 /)).toBeNull());
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "+" }));
  });
});
