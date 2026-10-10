import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { RpcError } from "../app/seam";
import { beforeOf } from "./reorder";
import { mountTodos, todo, todoCallsTo as callsTo, todoEvent } from "./testHarness";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

/** jsdom lays nothing out: rows are 28 high, 30 apart, in document order, inside a list spanning y 0 to 300. */
beforeEach(() => {
  const rowIndex = (el: HTMLElement) => [...el.parentElement!.children].indexOf(el);

  vi.spyOn(HTMLElement.prototype, "offsetTop", "get").mockImplementation(function (this: HTMLElement) {
    return this.classList.contains("todo-row") ? rowIndex(this) * 30 : 0;
  });
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(28);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({ top: 0, bottom: 300, left: 0, right: 500, width: 500, height: 300, x: 0, y: 0, toJSON: () => ({}) });
});

type Mounted = Awaited<ReturnType<typeof mountTodos>>;

const ids = () => [...document.querySelectorAll("[data-shelf-row] .todo-row-button")].map((row) => Number(/#(\d+)/.exec(row.textContent ?? "")![1]));

const buttonOf = (id: number) => screen.getByRole<HTMLButtonElement>("button", { name: new RegExp(`^(idle|blocked) #${id} `) });

const refetch = ({ app, store }: Mounted) => app.emit(todoEvent("todo.updated", { ...store.todos[0]! }));

const pointer = (target: Element | Window, type: string, init: MouseEventInit) => fireEvent(target, new MouseEvent(type, { bubbles: true, button: 0, ...init }));

/** A drag of `id` from y `from` to y `to`, released at `to`. */
const drag = (id: number, from: number, to: number, release = true) => {
  pointer(buttonOf(id), "pointerdown", { clientY: from, clientX: 10 });
  pointer(window, "pointermove", { clientY: to, clientX: 10 });

  if (release) pointer(window, "pointerup", { clientY: to, clientX: 10 });
};

const three = () => mountTodos([todo(1), todo(2), todo(3), todo(4, { done: true })]);

describe("u155 reorder a Todo row", () => {
  it("u155_dragging_the_third_row_above_the_first_calls_reorder_once_and_the_refetch_places_it", async () => {
    const mounted = await three();

    await screen.findByText(/todo 3/);
    drag(3, 70, 5);
    await waitFor(() => expect(callsTo(mounted.app, "todo.reorder")).toEqual([{ method: "todo.reorder", params: { id: 3, before: 1 } }]));
    expect(ids()).toEqual([1, 2, 3]);
    refetch(mounted);
    await waitFor(() => expect(ids()).toEqual([3, 1, 2]));
  });

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
    fireEvent.keyDown(buttonOf(3), { key: "ArrowDown", altKey: true });
    fireEvent.keyDown(buttonOf(1), { key: "ArrowUp", altKey: true });
    fireEvent.keyDown(buttonOf(2), { key: "ArrowDown" });
    expect(callsTo(mounted.app, "todo.reorder")).toEqual([]);
  });

  it("u155_the_last_row_moved_down_sends_the_todo_that_follows_the_list", async () => {
    const mounted = await three();

    await screen.findByText(/todo 3/);
    fireEvent.keyDown(buttonOf(2), { key: "ArrowDown", altKey: true });
    await waitFor(() => expect(callsTo(mounted.app, "todo.reorder")).toEqual([{ method: "todo.reorder", params: { id: 2, before: 4 } }]));
  });

  it("u155_escape_cancels_a_drag_and_a_release_outside_the_list_cancels_it", async () => {
    const mounted = await three();

    await screen.findByText(/todo 3/);
    drag(3, 70, 5, false);
    fireEvent.keyDown(window, { key: "Escape" });
    pointer(window, "pointerup", { clientY: 5, clientX: 10 });
    drag(3, 70, 5, false);
    pointer(window, "pointerup", { clientY: 5, clientX: 900 });
    expect(callsTo(mounted.app, "todo.reorder")).toEqual([]);
    expect(ids()).toEqual([1, 2, 3]);
  });

  it("u155_a_release_over_the_rows_own_place_calls_nothing", async () => {
    const mounted = await three();

    await screen.findByText(/todo 3/);
    drag(3, 70, 62);
    expect(callsTo(mounted.app, "todo.reorder")).toEqual([]);
  });

  it("u155_a_3px_press_is_a_click_that_opens_the_drawer_and_calls_nothing", async () => {
    const mounted = await three();

    await screen.findByText(/todo 3/);
    drag(2, 40, 43);
    fireEvent.click(buttonOf(2));
    await screen.findByRole("complementary", { name: "drawer" });
    expect(callsTo(mounted.app, "todo.reorder")).toEqual([]);
  });

  it("u155_a_drag_opens_no_drawer", async () => {
    await three();

    await screen.findByText(/todo 3/);
    drag(3, 70, 5);
    fireEvent.click(buttonOf(3));
    expect(screen.queryByRole("complementary", { name: "drawer" })?.textContent ?? "").not.toMatch(/todo 3/);
  });

  it("u155_the_done_fold_takes_no_drag_and_no_alt_key", async () => {
    const mounted = await three();

    fireEvent.click(await screen.findByRole("button", { name: /1 done/ }));

    const done = await screen.findByRole("button", { name: /^done #4 / });

    expect(done.getAttribute("aria-description")).toBeNull();
    fireEvent.keyDown(done, { key: "ArrowUp", altKey: true });
    pointer(done, "pointerdown", { clientY: 100 });
    pointer(window, "pointermove", { clientY: 5 });
    pointer(window, "pointerup", { clientY: 5 });
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
    expect(buttonOf(1).textContent?.trim()).toBe("#1 todo 1");
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
