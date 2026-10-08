import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import type { Actor } from "@contracts/Actor";
import type { RailNode } from "@contracts/agent/RailNode";
import type { Pad } from "@contracts/pad/Pad";
import type { Todo } from "@contracts/todo/Todo";
import { DrawerHost } from "../../drawer/DrawerHost";
import { Keys } from "../../keys/Keys";
import { Pads } from "../../pads/Pads";
import { ConnectedProjectContext, connectProject } from "../../state/connectedProject";
import { createFakeApp } from "../../testing/fakeApp";
import { agent, event, NOW } from "../../testing/nodes";
import { padHandlers, todoHandlers } from "../../testing/stores";
import type { PadStore, TodoStore } from "../../testing/stores";
import { Todos } from "../../todos/Todos";
import { todo } from "../../todos/testHarness";
import { Rail } from "../Rail";
import { rowOf } from "../railFixture";

afterEach(cleanup);

const by = (id: string): Actor => ({ kind: "agent", id, parent: null });

const padOf = (name: string, who: Actor): Pad => ({ name, owner: who, text: "", updated_at: 0 });

const mount = async (tree: RailNode[], todos: Todo[], pads: Pad[] = []) => {
  const padState: PadStore = { pads, history: [] };
  const store: TodoStore = { todos };
  const app = createFakeApp();
  app.handlers["rail.tree"] = () => tree;
  Object.assign(app.handlers, padHandlers(padState, (data) => app.emit(event(data))));
  Object.assign(app.handlers, todoHandlers(store, (data) => app.emit(event(data))));

  const connected = await connectProject(app, { name: "p", path: "/p" }, () => true, () => NOW);

  render(() => (
    <ConnectedProjectContext.Provider value={connected}>
      <Keys />
      <Rail />
      <Pads />
      <Todos />
      <DrawerHost drawer={connected.drawer} reducedMotion={() => true} />
    </ConnectedProjectContext.Provider>
  ));

  await connected.rail.settled();

  return { app, store, connected };
};

const TREE = [agent("a1", "working", "x", { name: "auth" }), agent("a2", "working", "x", { name: "docs", order: 1 })];

const control = (row: HTMLElement) => within(row).queryByRole("button", { name: /pads$/ });

const shelf = () => within(screen.getByRole("region", { name: "todos" }));

const opened = async () => {
  const button = await waitFor(() => control(rowOf("auth")) ?? Promise.reject(new Error("no control")));

  fireEvent.click(button);

  return button;
};

describe("u60 an Agent's Todos under it", () => {
  it("u60_the_control_counts_pads_and_open_todos_together", async () => {
    await mount(TREE, [todo(1, { creator: by("a1") }), todo(2, { creator: by("a1"), done: true })], [padOf("n", by("a1"))]);

    await waitFor(() => expect(control(rowOf("auth"))?.textContent?.trim()).toBe("2"));
    expect(control(rowOf("docs"))).toBeNull();
  });

  it("u60_expanded_the_pads_come_first_then_the_open_todos_in_id_order", async () => {
    await mount(
      TREE,
      [todo(5, { creator: by("a1") }), todo(3, { creator: by("a1"), blockers: [4], blocked: true }), todo(4, { creator: by("a1") })],
      [padOf("notes", by("a1"))],
    );
    await opened();

    const group = screen.getByTitle("notes").closest<HTMLElement>("[data-pads]")!;

    expect(group.style.paddingLeft).toBe("2ch");
    expect([...group.querySelectorAll("[data-todo], .rail-pad")].map((row) => row.getAttribute("data-todo") ?? "pad")).toEqual(["pad", "3", "4", "5"]);
    expect(within(group).getByText(/waits on/).textContent).toContain("#4");
    expect(within(group).getByText(/#4 todo 4/)).toBeTruthy();
  });

  it("u60_a_todo_of_an_agent_on_the_rail_is_not_in_the_shelf_but_the_others_stay", async () => {
    await mount(TREE, [
      todo(1, { creator: by("a1") }),
      todo(2),
      todo(3, { creator: by("gone") }),
      todo(4, { creator: { kind: "ext", id: "x", parent: null } }),
      todo(5, { creator: by("a1"), done: true }),
    ]);

    await shelf().findByText(/#2 todo 2/);

    expect(shelf().queryByText(/#1 todo 1/)).toBeNull();
    expect(shelf().getByText(/#3 todo 3/)).toBeTruthy();
    expect(shelf().getByText(/#4 todo 4/)).toBeTruthy();
    expect(shelf().getByText("1 done")).toBeTruthy();
  });

  it("u60_a_click_on_a_todo_opens_its_drawer_with_a_created_by_line", async () => {
    const { connected } = await mount(TREE, [todo(1, { creator: by("a1") })]);

    await opened();
    fireEvent.click(screen.getByText(/#1 todo 1/));

    const drawer = await screen.findByLabelText("drawer");

    expect(await within(drawer).findByText("created by auth")).toBeTruthy();
    expect(connected.rail.selected()).toBeNull();
  });

  it("u60_created_by_reads_you_for_the_user_and_the_id_for_an_agent_off_the_rail", async () => {
    await mount(TREE, [todo(1), todo(2, { creator: by("gone") })]);

    fireEvent.click(await shelf().findByText(/#1 todo 1/));
    expect(await within(await screen.findByLabelText("drawer")).findByText("created by you")).toBeTruthy();

    fireEvent.click(shelf().getByText(/#2 todo 2/));
    expect(await within(screen.getByLabelText("drawer")).findByText("created by gone")).toBeTruthy();
  });

  it("u60_complete_works_on_a_todo_under_its_agent", async () => {
    const { app } = await mount(TREE, [todo(1, { creator: by("a1") })]);

    await opened();

    const row = screen.getByText(/#1 todo 1/).closest<HTMLElement>("[data-todo]")!;

    fireEvent.mouseEnter(row);
    fireEvent.click(within(row).getByRole("button", { name: "complete" }));

    await waitFor(() => expect(app.calls.filter((call) => call.method === "todo.complete")).toEqual([{ method: "todo.complete", params: { id: 1 } }]));
    await waitFor(() => expect(control(rowOf("auth"))).toBeNull());
  });

  it("u60_the_rail_and_the_shelf_share_one_todo_list_and_the_rows_leave_the_agents_alone", async () => {
    const { app } = await mount(TREE, [todo(1, { creator: by("a1") })]);

    await opened();

    expect(app.calls.filter((call) => call.method === "todo.list")).toHaveLength(1);
    expect([...document.querySelectorAll("[role=tree] [data-id]")].map((row) => row.getAttribute("data-id"))).toEqual(["a1", "a2"]);
  });
});
