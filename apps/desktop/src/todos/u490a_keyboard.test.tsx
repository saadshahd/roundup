import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import type { Actor } from "@contracts/Actor";
import type { Todo } from "@contracts/todo/Todo";
import { DrawerHost } from "../drawer/DrawerHost";
import { Keys } from "../keys/Keys";
import { rowOf } from "../rail/railFixture";
import { Rail } from "../rail/Rail";
import { RpcError } from "../app/seam";
import { ConnectedProjectContext, connectProject } from "../state/connectedProject";
import { createFakeApp } from "../testing/fakeApp";
import { agent, event, NOW } from "../testing/nodes";
import { showEverything, withScopeRow } from "../testing/showEverything";
import { todoHandlers } from "../testing/stores";
import type { TodoStore } from "../testing/stores";
import { beforeOf } from "./beforeOf";
import { Todos } from "./Todos";
import { todo, todoCallsTo as callsTo, todoEvent } from "./testHarness";

afterEach(cleanup);

const ids = (root: ParentNode) => [...root.querySelectorAll(".todo-row-button")].map((row) => Number(/#(\d+)/.exec(row.getAttribute("aria-label") ?? "")![1]));

const by = (id: string): Actor => ({ kind: "agent", id, parent: null });

const mountNested = async (todos: Todo[]) => {
  const store: TodoStore = { todos };
  const app = createFakeApp();

  app.handlers["rail.tree"] = () => withScopeRow([agent("a1", "working", "x", { name: "auth" })]);
  Object.assign(app.handlers, todoHandlers(store, (data) => app.emit(event(data))));

  const connected = await connectProject(app, { name: "p", path: "/p" }, () => true, () => NOW);

  await showEverything(connected);
  render(() => (
    <ConnectedProjectContext.Provider value={connected}>
      <Keys />
      <Rail />
      <Todos />
      <DrawerHost drawer={connected.drawer} reducedMotion={() => true} />
    </ConnectedProjectContext.Provider>
  ));
  await connected.rail.settled();
  fireEvent.click(await waitFor(() => within(rowOf("auth")).getByRole("button", { name: /todos$/ })));

  return { app, store };
};

describe("u490a an Agent's list orders within itself", () => {
  it("u490a_nested_order_boundary", async () => {
    // The Shelf holds #1, #2 and #6; the Agent's list is #3, #4, #5; #6 follows the list in `todo.list`.
    const { app } = await mountNested([todo(1), todo(2), todo(3, { creator: by("a1") }), todo(4, { creator: by("a1") }), todo(5, { creator: by("a1") }), todo(6)]);
    const group = await waitFor(() => document.querySelector<HTMLElement>("[data-pads]") ?? Promise.reject(new Error("no list")));

    await waitFor(() => expect(ids(group)).toEqual([3, 4, 5]));

    const key = (id: number, key: string) => {
      const row = within(group).getByRole("button", { name: new RegExp(`#${id} `) });
      row.focus();
      fireEvent.keyDown(row, { key, altKey: true });
    };

    key(3, "ArrowUp");
    key(5, "ArrowDown");
    expect(callsTo(app, "todo.reorder")).toEqual([]);
    key(4, "ArrowDown");
    await waitFor(() => expect(callsTo(app, "todo.reorder")).toEqual([{ method: "todo.reorder", params: { id: 4, before: 6 } }]));
    key(5, "ArrowUp");
    await waitFor(() => expect(callsTo(app, "todo.reorder")).toHaveLength(2));
    expect(callsTo(app, "todo.reorder")[1]).toEqual({ method: "todo.reorder", params: { id: 5, before: 4 } });
    expect(ids(screen.getByRole("region", { name: "Todos" }))).toEqual([1, 2, 6]);
  });

  it("u490a_keyboard_reorders_moves_focus_with_the_row_and_shows_a_failure", async () => {
    const { app } = await mountNested([todo(3, { creator: by("a1") }), todo(4, { creator: by("a1") }), todo(5, { creator: by("a1") })]);
    const group = await waitFor(() => document.querySelector<HTMLElement>("[data-pads]") ?? Promise.reject(new Error("no list")));
    const row = (id: number) => within(group).getByRole<HTMLButtonElement>("button", { name: new RegExp(`#${id} `) });

    await waitFor(() => expect(ids(group)).toEqual([3, 4, 5]));
    row(4).focus();
    fireEvent.keyDown(row(4), { key: "ArrowUp", altKey: true });
    await waitFor(() => expect(callsTo(app, "todo.reorder")).toEqual([{ method: "todo.reorder", params: { id: 4, before: 3 } }]));
    app.emit(todoEvent("todo.updated", todo(4, { creator: by("a1") })));
    await waitFor(() => expect(ids(group)).toEqual([4, 3, 5]));
    expect(document.activeElement).toBe(row(4));

    app.handlers["todo.reorder"] = () => {
      throw new RpcError(-32000, "no room");

    };

    fireEvent.keyDown(row(4), { key: "ArrowDown", altKey: true });
    await within(group).findByText(/no room/);
    expect(ids(group)).toEqual([4, 3, 5]);
    expect(document.activeElement).toBe(row(4));
  });

  it("u490a_nested_order_boundary_translates_to_the_next_row", () => {
    const all = [1, 2, 3, 4, 5].map((id) => todo(id));

    expect(beforeOf(all, [2, 4], 4, 0)).toBe(2);
    expect(beforeOf(all, [2, 4], 2, 1)).toBe(5);
    expect(beforeOf(all.slice(0, 4), [2, 4], 2, 1)).toBeNull();
  });
});
