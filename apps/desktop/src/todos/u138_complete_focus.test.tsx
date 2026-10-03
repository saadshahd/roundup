import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { RpcError } from "../app/seam";
import { mountTodos, todo, todoCallsTo, todoEvent } from "./testHarness";

afterEach(cleanup);

const rowButton = (id: number) => screen.findByRole("button", { name: new RegExp(`#${id} `) });

type Mounted = Awaited<ReturnType<typeof mountTodos>>;

/** Focuses the row, so `complete` shows (U35), then focuses `complete` and presses it as Enter or Space does: a click; the Daemon's `todo.updated` then reaches the list. */
const completeByKeyboard = async ({ app, store }: Mounted, id: number) => {
  (await rowButton(id)).focus();
  const complete = await screen.findByRole("button", { name: "complete" });

  complete.focus();
  fireEvent.click(complete);
  await waitFor(() => expect(todoCallsTo(app, "todo.complete")).toHaveLength(1));
  app.emit(todoEvent("todo.updated", { ...store.todos.find((candidate) => candidate.id === id)! }));
};

describe("u138 completing a Todo by keyboard keeps focus", () => {
  it("u138_completing_a_middle_todo_focuses_the_next_open_row", async () => {
    const mounted = await mountTodos([todo(1), todo(2), todo(3)]);

    await completeByKeyboard(mounted, 2);

    await waitFor(() => expect(screen.queryByText(/#2 /)).toBeNull());
    expect(document.activeElement).toBe(await rowButton(3));
  });

  it("u138_completing_the_last_open_todo_focuses_the_previous_row", async () => {
    const mounted = await mountTodos([todo(1), todo(2), todo(3)]);

    await completeByKeyboard(mounted, 3);

    await waitFor(() => expect(screen.queryByText(/#3 /)).toBeNull());
    expect(document.activeElement).toBe(await rowButton(2));
  });

  it("u138_completing_the_only_open_todo_focuses_the_new_todo_button", async () => {
    const mounted = await mountTodos([todo(1)]);

    await completeByKeyboard(mounted, 1);

    await waitFor(() => expect(screen.queryByText(/#1 /)).toBeNull());
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "+" }));
  });

  it("u138_a_failed_complete_keeps_focus_on_complete", async () => {
    const mounted = await mountTodos([todo(1), todo(2)]);
    mounted.app.handlers["todo.complete"] = () => {
      throw new RpcError(-32000, "boom");
    };
    (await rowButton(1)).focus();
    const complete = await screen.findByRole("button", { name: "complete" });

    complete.focus();
    fireEvent.click(complete);

    await screen.findByText(/boom/);
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "complete" }));
  });
});
