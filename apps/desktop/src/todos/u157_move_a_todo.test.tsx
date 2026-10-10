import { cleanup, fireEvent, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { RpcError } from "../app/seam";
import { workstream } from "../testing/nodes";
import { mountTodos, todo, todoCallsTo as callsTo, todoEvent, todoRowOf as rowOf } from "./testHarness";

afterEach(cleanup);

const workstreams = () => [workstream("r1", { name: "alpha", order: 0 }), workstream("r2", { name: "beta", order: 1 })];

const drawer = () => screen.getByRole("complementary", { name: "drawer" });

const homeOf = (id: number) => rowOf(id).querySelector("[data-todo-home]")?.textContent;

const settle = async (mounted: Awaited<ReturnType<typeof mountTodos>>, text = /todo 2/) => {
  await mounted.connected.rail.settled();
  await screen.findByText(text);
};

/** The Daemon's side of a move: the store changes, `todo.updated` arrives. */
const moveInStore = (mounted: Awaited<ReturnType<typeof mountTodos>>) => {
  mounted.app.handlers["todo.move"] = ({ id, home }) => {
    mounted.store.todos = mounted.store.todos.map((each) => (each.id === id ? { ...each, home } : each));
    const moved = mounted.store.todos.find((each) => each.id === id)!;

    mounted.app.emit(todoEvent("todo.updated", moved));

    return moved;
  };
};

describe("u157 move a Todo from the row and the Drawer", () => {
  it("u157_the_row_offers_project_root_and_the_workstreams_not_the_current_home_and_moves_by_pointer", async () => {
    const mounted = await mountTodos([todo(2)], true, workstreams());
    moveInStore(mounted);
    await settle(mounted);

    fireEvent.mouseEnter(rowOf(2));
    const word = screen.getByRole("button", { name: "move #2" });

    fireEvent.click(word);
    const offers = within(screen.getByRole("group", { name: "homes for #2" })).getAllByRole("button").map((each) => each.textContent);

    expect(offers).toEqual(["alpha", "beta"]);

    fireEvent.click(screen.getByRole("button", { name: "beta" }));

    await waitFor(() => expect(homeOf(2)).toBe("in beta"));
    expect(callsTo(mounted.app, "todo.move")).toEqual([{ method: "todo.move", params: { id: 2, home: "r2" } }]);
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "move #2" }));
    expect(screen.queryByRole("group", { name: "homes for #2" })).toBeNull();
  });

  it("u157_the_row_moves_back_to_project_root_by_keyboard_and_the_current_workstream_is_left_out", async () => {
    const mounted = await mountTodos([todo(2, { home: "r1" })], true, workstreams());
    moveInStore(mounted);
    await settle(mounted);
    expect(homeOf(2)).toBe("in alpha");

    screen.getByRole("button", { name: /todo 2/ }).focus();
    const word = screen.getByRole("button", { name: "move #2" });

    word.focus();
    fireEvent.keyDown(word, { key: "Enter" });
    fireEvent.click(word);
    const offers = within(screen.getByRole("group", { name: "homes for #2" })).getAllByRole("button");

    expect(offers.map((each) => each.textContent)).toEqual(["project root", "beta"]);

    offers[0]!.focus();
    fireEvent.click(offers[0]!);

    await waitFor(() => expect(homeOf(2)).toBe("in project root"));
    expect(callsTo(mounted.app, "todo.move")).toEqual([{ method: "todo.move", params: { id: 2, home: null } }]);
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "move #2" }));
  });

  it("u157_esc_closes_the_list_and_focus_returns_to_the_move_word_without_closing_the_drawer", async () => {
    const mounted = await mountTodos([todo(2)], true, workstreams());
    await settle(mounted);
    fireEvent.click(screen.getByRole("button", { name: /todo 2/ }));
    await within(drawer()).findByText("created by you");

    const word = within(drawer()).getByRole("button", { name: "move #2" });

    fireEvent.click(word);
    const offer = within(drawer()).getByRole("button", { name: "alpha" });

    offer.focus();
    fireEvent.keyDown(offer, { key: "Escape" });

    expect(within(drawer()).queryByRole("group", { name: "homes for #2" })).toBeNull();
    expect(document.activeElement).toBe(word);
    expect(drawer().getAttribute("data-open")).toBe("true");
    expect(callsTo(mounted.app, "todo.move")).toEqual([]);
  });

  it("u157_a_click_outside_closes_the_list", async () => {
    const mounted = await mountTodos([todo(2)], true, workstreams());
    await settle(mounted);
    fireEvent.mouseEnter(rowOf(2));
    fireEvent.click(screen.getByRole("button", { name: "move #2" }));
    expect(screen.getByRole("group", { name: "homes for #2" })).toBeTruthy();

    fireEvent.click(document.body);

    expect(screen.queryByRole("group", { name: "homes for #2" })).toBeNull();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "move #2" }));
  });

  it("u157_a_click_outside_after_the_pointer_leaves_keeps_focus_on_the_word", async () => {
    const mounted = await mountTodos([todo(2)], true, workstreams());
    await settle(mounted);
    fireEvent.mouseEnter(rowOf(2));
    const word = screen.getByRole("button", { name: "move #2" });
    word.focus();
    fireEvent.click(word);
    fireEvent.mouseLeave(rowOf(2));
    word.blur();
    expect(screen.getByRole("group", { name: "homes for #2" })).toBeTruthy();

    fireEvent.click(document.body);

    expect(screen.queryByRole("group", { name: "homes for #2" })).toBeNull();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "move #2" }));
  });

  it("u157_the_drawer_moves_a_todo_by_pointer_and_the_line_follows_the_event", async () => {
    const mounted = await mountTodos([todo(2)], true, workstreams());
    moveInStore(mounted);
    await settle(mounted);
    fireEvent.click(screen.getByRole("button", { name: /todo 2/ }));
    await within(drawer()).findByText("created by you");

    expect(within(drawer()).getByText(/^in project root/)).toBeTruthy();
    fireEvent.click(within(drawer()).getByRole("button", { name: "move #2" }));
    expect(within(drawer()).getAllByRole("button").filter((each) => each.classList.contains("todo-home-offer")).map((each) => each.textContent)).toEqual(["alpha", "beta"]);
    fireEvent.click(within(drawer()).getByRole("button", { name: "alpha" }));

    await waitFor(() => expect(homeOf(2)).toBe("in alpha"));
    expect(within(drawer()).getByText(/^in alpha/)).toBeTruthy();
    expect(callsTo(mounted.app, "todo.move")).toEqual([{ method: "todo.move", params: { id: 2, home: "r1" } }]);
    expect(callsTo(mounted.app, "todo.get")).toHaveLength(1);
    expect(document.activeElement).toBe(within(drawer()).getByRole("button", { name: "move #2" }));
  });

  it("u157_the_drawer_moves_back_by_keyboard", async () => {
    const mounted = await mountTodos([todo(2, { home: "r2" })], true, workstreams());
    moveInStore(mounted);
    await settle(mounted);
    fireEvent.click(screen.getByRole("button", { name: /todo 2/ }));
    await within(drawer()).findByText("created by you");
    const word = within(drawer()).getByRole("button", { name: "move #2" });

    word.focus();
    fireEvent.click(word);
    const root = within(drawer()).getByRole("button", { name: "project root" });

    root.focus();
    fireEvent.click(root);

    await waitFor(() => expect(homeOf(2)).toBe("in project root"));
    expect(callsTo(mounted.app, "todo.move")).toEqual([{ method: "todo.move", params: { id: 2, home: null } }]);
    expect(document.activeElement).toBe(word);
  });

  it("u157_a_failed_move_from_the_row_shows_the_message_and_keeps_the_home_line", async () => {
    const mounted = await mountTodos([todo(2, { blocked: true, blockers: [1] }), todo(1)], true, workstreams());
    mounted.app.handlers["todo.move"] = () => {
      throw new RpcError(-32001, "workstream gone");
    };

    await settle(mounted);
    fireEvent.mouseEnter(rowOf(2));
    fireEvent.click(screen.getByRole("button", { name: "move #2" }));
    fireEvent.click(screen.getByRole("button", { name: "alpha" }));

    expect((await within(rowOf(2)).findByText(/workstream gone/)).textContent).toBe("workstream gone");
    expect(homeOf(2)).toBe("in project root");
    expect(rowOf(2).querySelector("[data-todo-waits]")).toBeNull();
  });

  it("u157_a_failed_move_from_the_drawer_shows_the_message_inside_the_open_list", async () => {
    const mounted = await mountTodos([todo(2)], true, workstreams());
    mounted.app.handlers["todo.move"] = () => {
      throw new RpcError(-32602, "bad home");
    };

    await settle(mounted);
    fireEvent.click(screen.getByRole("button", { name: /todo 2/ }));
    await within(drawer()).findByText("created by you");
    fireEvent.click(within(drawer()).getByRole("button", { name: "move #2" }));
    fireEvent.click(within(drawer()).getByRole("button", { name: "alpha" }));

    const list = within(drawer()).getByRole("group", { name: "homes for #2" });

    expect((await within(list).findByText(/bad home/)).textContent).toBe("bad home");
    expect(within(drawer()).getByText(/^in project root/)).toBeTruthy();
  });

  it("u157_a_done_todos_drawer_moves_it", async () => {
    const mounted = await mountTodos([todo(2, { done: true }), todo(3)], true, workstreams());
    moveInStore(mounted);
    await settle(mounted, /todo 3/);
    fireEvent.click(await screen.findByRole("button", { name: /1 done/ }));
    fireEvent.click(screen.getByRole("button", { name: /todo 2/ }));
    await within(drawer()).findByText("created by you");

    expect(within(drawer()).getByText(/^in project root/)).toBeTruthy();
    fireEvent.click(within(drawer()).getByRole("button", { name: "move #2" }));
    fireEvent.click(within(drawer()).getByRole("button", { name: "beta" }));

    await waitFor(() => expect(within(drawer()).getByText(/^in beta/)).toBeTruthy());
    expect(callsTo(mounted.app, "todo.move")).toEqual([{ method: "todo.move", params: { id: 2, home: "r2" } }]);
  });

  it("u157_choosing_from_the_row_reads_no_todo", async () => {
    const mounted = await mountTodos([todo(2)], true, workstreams());
    moveInStore(mounted);
    await settle(mounted);
    fireEvent.mouseEnter(rowOf(2));
    fireEvent.click(screen.getByRole("button", { name: "move #2" }));
    fireEvent.click(screen.getByRole("button", { name: "alpha" }));

    await waitFor(() => expect(homeOf(2)).toBe("in alpha"));
    expect(callsTo(mounted.app, "todo.get")).toEqual([]);
  });

  it("u157_the_row_keeps_its_place_and_the_shelf_lists_every_todo_whatever_its_home", async () => {
    const mounted = await mountTodos([todo(1), todo(2), todo(3, { home: "r1" })], true, workstreams());
    moveInStore(mounted);
    await settle(mounted);
    fireEvent.mouseEnter(rowOf(2));
    fireEvent.click(screen.getByRole("button", { name: "move #2" }));
    fireEvent.click(screen.getByRole("button", { name: "beta" }));

    await waitFor(() => expect(homeOf(2)).toBe("in beta"));
    expect([...document.querySelectorAll("[data-shelf-row]")].map((row) => row.getAttribute("data-id"))).toEqual(["1", "2", "3"]);
  });

  it("u157_a_workstream_with_a_60_character_name_is_offered_in_full", async () => {
    const long = "r".repeat(60);
    const mounted = await mountTodos([todo(2)], true, [workstream("r1", { name: long })]);
    await settle(mounted);
    fireEvent.mouseEnter(rowOf(2));
    fireEvent.click(screen.getByRole("button", { name: "move #2" }));

    const offer = screen.getByRole("button", { name: long });

    expect(offer.textContent).toBe(long);
    expect(getComputedStyle(offer).overflowWrap).toBe("anywhere");
  });

  it("u157_the_home_line_changes_only_when_todo_updated_arrives", async () => {
    const mounted = await mountTodos([todo(2)], true, workstreams());
    await settle(mounted);
    mounted.app.handlers["todo.move"] = ({ id, home }) => {
      mounted.store.todos = mounted.store.todos.map((each) => (each.id === id ? { ...each, home } : each));

      return mounted.store.todos[0]!;
    };

    fireEvent.mouseEnter(rowOf(2));
    fireEvent.click(screen.getByRole("button", { name: "move #2" }));
    fireEvent.click(screen.getByRole("button", { name: "alpha" }));

    await waitFor(() => expect(callsTo(mounted.app, "todo.move")).toHaveLength(1));
    expect(homeOf(2)).toBe("in project root");

    mounted.app.emit(todoEvent("todo.updated", mounted.store.todos[0]!));

    await waitFor(() => expect(homeOf(2)).toBe("in alpha"));
  });
});
