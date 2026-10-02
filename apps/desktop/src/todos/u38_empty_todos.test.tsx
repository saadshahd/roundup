import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { connectProject, ConnectedProjectContext } from "../state/connectedProject";
import { createFakeApp } from "../testing/fakeApp";
import { mountTodos, todo } from "./testHarness";
import { Todos } from "./Todos";

afterEach(cleanup);

describe("u38 empty Todos", () => {
  it("u38_an_open_project_with_no_todos_reads_no_todos_yet", async () => {
    await mountTodos([]);

    expect(await screen.findByText("no todos yet")).toBeTruthy();
  });

  it("u38_no_todos_yet_never_flashes_before_the_first_todo_list_answers", async () => {
    const app = createFakeApp();
    const pending = Promise.withResolvers<ReturnType<typeof todo>[]>();

    app.handlers["todo.list"] = () => pending.promise;

    const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, () => 0);

    render(() => (
      <ConnectedProjectContext.Provider value={connected}>
        <Todos />
      </ConnectedProjectContext.Provider>
    ));

    expect(screen.queryByText("no todos yet")).toBeNull();

    pending.resolve([]);

    expect(await screen.findByText("no todos yet")).toBeTruthy();
  });

  it("u38_no_todos_yet_disappears_once_a_todo_exists", async () => {
    await mountTodos([todo(1)]);
    await screen.findByText(/todo 1/);

    expect(screen.queryByText("no todos yet")).toBeNull();
  });

  it("u38_a_failed_initial_todo_list_shows_its_message_not_no_todos_yet", async () => {
    const app = createFakeApp();

    app.handlers["todo.list"] = () => {
      throw new Error("daemon says no");
    };

    const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, () => 0);

    render(() => (
      <ConnectedProjectContext.Provider value={connected}>
        <Todos />
      </ConnectedProjectContext.Provider>
    ));

    expect((await screen.findByText(/daemon says no/)).textContent).toBe("✕ daemon says no");
    expect(screen.queryByText("no todos yet")).toBeNull();
  });
});
