import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { RpcError } from "../app/seam";
import { event } from "../testing/nodes";
import { mountTodos, todo, todoCallsTo } from "./testHarness";
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
