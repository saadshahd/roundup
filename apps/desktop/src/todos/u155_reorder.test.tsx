import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { RpcError } from "../app/seam";
import { beforeOf } from "./beforeOf";
import { mountTodos, todo, todoCallsTo as callsTo, todoEvent } from "./testHarness";

afterEach(cleanup);

type Mounted = Awaited<ReturnType<typeof mountTodos>>;

const ids = () => [...document.querySelectorAll("[data-shelf-row] .todo-row-button")].map((row) => Number(/#(\d+)/.exec(row.getAttribute("aria-label") ?? "")![1]));

const buttonOf = (id: number) => screen.getByRole<HTMLButtonElement>("button", { name: new RegExp(`^(Idle|Blocked) #${id} `) });

const refetch = ({ app, store }: Mounted) => app.emit(todoEvent("todo.updated", { ...store.todos[0]! }));

const three = () => mountTodos([todo(1), todo(2), todo(3), todo(4, { done: true })]);

describe("u155 reorder a Todo row", () => {
  it("u155_alt_down_on_the_first_row_calls_reorder_before_the_third_and_focus_stays_on_it", async () => {
    const mounted = await three();

    await screen.findByText(/todo 3/);
    buttonOf(1).focus();
    fireEvent.keyDown(buttonOf(1), { key: "ArrowDown", altKey: true });
    await waitFor(() => expect(callsTo(mounted.app, "todo.reorder")).toEqual([{ method: "todo.reorder", params: { id: 1, before: 3 } }]));
    refetch(mounted);
    await waitFor(() => expect(ids()).toEqual([2, 1, 3]));
    expect(document.activeElement).toBe(buttonOf(1));
  });

  it("u155_alt_down_on_the_last_row_and_alt_up_on_the_first_call_nothing", async () => {
    const mounted = await three();

    await screen.findByText(/todo 3/);
    buttonOf(3).focus();
    fireEvent.keyDown(buttonOf(3), { key: "ArrowDown", altKey: true });
    buttonOf(1).focus();
    fireEvent.keyDown(buttonOf(1), { key: "ArrowUp", altKey: true });
    buttonOf(2).focus();
    fireEvent.keyDown(buttonOf(2), { key: "ArrowDown" });
    expect(callsTo(mounted.app, "todo.reorder")).toEqual([]);
  });

  it("u155_the_last_row_moved_down_sends_the_todo_that_follows_the_list", async () => {
    const mounted = await three();

    await screen.findByText(/todo 3/);
    buttonOf(2).focus();
    fireEvent.keyDown(buttonOf(2), { key: "ArrowDown", altKey: true });
    await waitFor(() => expect(callsTo(mounted.app, "todo.reorder")).toEqual([{ method: "todo.reorder", params: { id: 2, before: 4 } }]));
  });

  it("u155_the_done_fold_takes_no_alt_key", async () => {
    const mounted = await three();

    fireEvent.click(await screen.findByRole("button", { name: /1 done/ }));

    const done = await screen.findByRole("button", { name: /^Done #4 / });

    expect(done.getAttribute("aria-description")).toBeNull();
    done.focus();
    fireEvent.keyDown(done, { key: "ArrowUp", altKey: true });
    expect(callsTo(mounted.app, "todo.reorder")).toEqual([]);
  });

  it("u155_a_failed_call_shows_the_message_in_place_of_waits_on_and_keeps_the_order", async () => {
    const mounted = await three();

    mounted.app.handlers["todo.reorder"] = () => {
      throw new RpcError(-32000, "no room");
    };

    await screen.findByText(/todo 3/);
    buttonOf(1).focus();
    fireEvent.keyDown(buttonOf(1), { key: "ArrowDown", altKey: true });
    await screen.findByText(/no room/);
    expect(ids()).toEqual([1, 2, 3]);
  });

  it("u155_the_row_button_says_how_it_moves_and_shows_nothing_new_at_rest", async () => {
    await three();

    await screen.findByText(/todo 3/);
    expect(buttonOf(1).getAttribute("aria-description")).toBe("Alt+Up or Alt+Down moves it");
    expect(buttonOf(1).textContent?.trim()).toBe("todo 1");
  });
});

describe("u155 where a moved row goes", () => {
  const all = [1, 2, 3, 4, 5].map((id) => todo(id));

  it("u155_a_move_under_an_agent_sends_the_next_row_of_its_list_or_the_todo_after_the_list", () => {
    expect(beforeOf(all, [2, 4], 4, 0)).toBe(2);
    expect(beforeOf(all, [2, 4], 2, 1)).toBe(5);
    expect(beforeOf(all.slice(0, 4), [2, 4], 2, 1)).toBeNull();
  });
});
