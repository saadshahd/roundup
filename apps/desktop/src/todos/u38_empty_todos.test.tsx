import { cleanup, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { connectProject, ConnectedProjectContext } from "../state/connectedProject";
import { createFakeApp } from "../testing/fakeApp";
import { event } from "../testing/nodes";
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
    const { app, store } = await mountTodos([]);

    await screen.findByText("no todos yet");

    store.todos = [todo(1)];
    app.emit(event({ name: "todo.created", data: todo(1) }));

    await screen.findByText(/todo 1/);
    expect(screen.queryByText("no todos yet")).toBeNull();
  });

  it("u38_a_stale_first_todo_list_reply_never_sets_no_todos_yet_before_the_newest_one_lands", async () => {
    const app = createFakeApp();
    const replies: ReturnType<typeof Promise.withResolvers<ReturnType<typeof todo>[]>>[] = [];

    app.handlers["todo.list"] = () => {
      const reply = Promise.withResolvers<ReturnType<typeof todo>[]>();

      replies.push(reply);

      return reply.promise;
    };

    const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, () => 0);

    render(() => (
      <ConnectedProjectContext.Provider value={connected}>
        <Todos />
      </ConnectedProjectContext.Provider>
    ));

    await waitFor(() => expect(replies.length).toBe(1));

    app.emit(event({ name: "todo.created", data: todo(1) }));
    await waitFor(() => expect(replies.length).toBe(2));

    replies[0]?.resolve([]);
    await new Promise((done) => setTimeout(done, 0));

    expect(screen.queryByText("no todos yet")).toBeNull();

    replies[1]?.resolve([todo(1)]);

    expect(await screen.findByText(/todo 1/)).toBeTruthy();
    expect(screen.queryByText("no todos yet")).toBeNull();
  });

  it("u38_no_todos_yet_returns_once_the_last_todo_is_deleted", async () => {
    const { app, store } = await mountTodos([todo(1)]);

    await screen.findByText(/todo 1/);

    store.todos = [];
    app.emit(event({ name: "todo.deleted", data: { id: 1 } }));

    expect(await screen.findByText("no todos yet")).toBeTruthy();
    expect(screen.queryByText(/todo 1/)).toBeNull();
  });

  it("u38_a_retry_after_a_failed_todo_list_keeps_the_failure_until_the_reply_arrives", async () => {
    const app = createFakeApp();
    const retry = Promise.withResolvers<ReturnType<typeof todo>[]>();
    let calls = 0;

    app.handlers["todo.list"] = () => {
      calls += 1;

      return calls === 1 ? Promise.reject(new Error("boom")) : retry.promise;
    };

    const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, () => 0);

    render(() => (
      <ConnectedProjectContext.Provider value={connected}>
        <Todos />
      </ConnectedProjectContext.Provider>
    ));

    expect((await screen.findByText(/boom/)).textContent).toBe("✕ boom");

    app.emit(event({ name: "todo.created", data: todo(1) }));
    await waitFor(() => expect(calls).toBe(2));

    expect(screen.getByText("✕ boom")).toBeTruthy();
    expect(screen.queryByText("no todos yet")).toBeNull();

    retry.resolve([]);

    expect(await screen.findByText("no todos yet")).toBeTruthy();
    expect(screen.queryByText(/boom/)).toBeNull();
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
