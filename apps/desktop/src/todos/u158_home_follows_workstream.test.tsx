import { cleanup, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import type { RailNode } from "@contracts/agent/RailNode";
import { event, workstream } from "../testing/nodes";
import { mountTodos, todo, todoEvent, todoRowOf as rowOf } from "./testHarness";

afterEach(cleanup);

const homeOf = (id: number) => rowOf(id).querySelector("[data-todo-home]")?.textContent;

describe("u158 the Home line follows the Workstream", () => {
  it("u158_a_rename_changes_the_line_without_a_todo_event", async () => {
    let tree: RailNode[] = [workstream("r1", { name: "alpha" })];
    const mounted = await mountTodos([todo(2, { home: "r1" })], true, tree);

    mounted.app.handlers["rail.tree"] = () => tree;
    await screen.findByText(/todo 2/);
    expect(homeOf(2)).toBe("in alpha");

    tree = [workstream("r1", { name: "gamma" })];
    mounted.app.emit(event({ name: "rail.changed" }));

    await waitFor(() => expect(homeOf(2)).toBe("in gamma"));
  });

  it("u158_removing_the_workstream_then_todo_updated_gives_project_root", async () => {
    let tree: RailNode[] = [workstream("r1", { name: "alpha" })];
    const mounted = await mountTodos([todo(2, { home: "r1" })], true, tree);

    mounted.app.handlers["rail.tree"] = () => tree;
    await screen.findByText(/todo 2/);

    tree = [];
    mounted.app.emit(event({ name: "rail.changed" }));
    mounted.store.todos = [todo(2, { home: null })];
    mounted.app.emit(todoEvent("todo.updated", todo(2, { home: null })));

    await waitFor(() => expect(homeOf(2)).toBe("in project root"));
  });

  it("u158_an_unknown_home_id_reads_project_root_and_never_the_id", async () => {
    await mountTodos([todo(2, { home: "ghost" })]);
    await screen.findByText(/todo 2/);

    expect(homeOf(2)).toBe("in project root");
  });
});
