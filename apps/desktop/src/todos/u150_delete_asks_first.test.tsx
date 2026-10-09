import { cleanup, fireEvent, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { RpcError } from "../app/seam";
import { mountTodos, todo, todoCallsTo as callsTo } from "./testHarness";
import { USER } from "../testing/nodes";

afterEach(cleanup);

const drawer = () => screen.getByRole("complementary", { name: "drawer" });

const open = async () => {
  const mounted = await mountTodos([todo(5, { title: "session store" }), todo(7, { title: "other" })], true);
  fireEvent.click(await screen.findByRole("button", { name: /#5/ }));
  await within(drawer()).findByText("blocker");

  return mounted;
};

const ask = () => fireEvent.click(within(drawer()).getByText("delete"));

describe("u150 delete asks first (Todo)", () => {
  it("u150_choosing_delete_asks_in_place_and_calls_nothing", async () => {
    const { app } = await open();

    ask();

    expect([drawer().textContent?.includes("delete #5?"), within(drawer()).queryByText("complete"), callsTo(app, "todo.delete")]).toEqual([true, null, []]);
  });

  it("u150_confirming_calls_todo_delete_once_and_closes_with_no_deleted_line", async () => {
    const { app, connected } = await open();
    app.handlers["todo.delete"] = () => null;
    ask();

    fireEvent.click(within(drawer()).getByText("delete"));
    app.emit({ actor: USER, name: "todo.deleted", data: { id: 5 } });

    await waitFor(() => expect(connected.drawer.content()).toBeNull());
    expect([callsTo(app, "todo.delete"), screen.queryByText(/was deleted/)]).toEqual([[{ method: "todo.delete", params: { id: 5 } }], null]);
  });

  it("u150_the_row_leaves_the_list_after_todo_deleted", async () => {
    const { app, store } = await mountTodos([todo(5, { title: "session store" }), todo(7, { title: "other" })], true);
    app.handlers["todo.delete"] = () => {
      store.todos = store.todos.filter((each) => each.id !== 5);

      return null;
    };

    fireEvent.click(await screen.findByRole("button", { name: /#5/ }));
    await within(drawer()).findByText("blocker");
    ask();
    fireEvent.click(within(drawer()).getByText("delete"));
    app.emit({ actor: USER, name: "todo.deleted", data: { id: 5 } });

    await waitFor(() => expect(screen.queryByRole("button", { name: /#5/ })).toBeNull());
    expect(screen.getByRole("button", { name: /#7/ })).toBeTruthy();
  });

  it("u150_keep_calls_nothing_and_returns_focus_to_delete", async () => {
    const { app } = await open();
    ask();
    const keep = within(drawer()).getByText("keep");
    await waitFor(() => expect(document.activeElement).toBe(keep));

    fireEvent.click(keep);

    await waitFor(() => expect(document.activeElement).toBe(within(drawer()).getByText("delete")));
    expect([callsTo(app, "todo.delete"), within(drawer()).queryByText("keep")]).toEqual([[], null]);
  });

  it("u150_esc_closes_the_drawer_and_calls_nothing", async () => {
    const { app, connected } = await open();
    ask();

    fireEvent.keyDown(document.body, { key: "Escape" });

    expect([connected.drawer.content(), callsTo(app, "todo.delete")]).toEqual([null, []]);
  });

  it("u150_an_error_keeps_the_drawer_and_the_item", async () => {
    const { app, connected } = await open();
    app.handlers["todo.delete"] = () => Promise.reject(new RpcError(-32001, "not found"));
    ask();

    fireEvent.click(within(drawer()).getByText("delete"));

    expect([(await within(drawer()).findByText(/not found/)).textContent, connected.drawer.content() === null]).toEqual(["not found", false]);
    expect(screen.getAllByRole("button", { name: /#5/ }).length).toBeGreaterThan(0);
  });
});
