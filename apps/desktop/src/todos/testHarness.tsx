import { render } from "@solidjs/testing-library";
import type { Actor } from "@contracts/Actor";
import type { Event as DaemonEvent } from "@contracts/Event";
import type { Todo } from "@contracts/todo/Todo";
import { Layout } from "../app/Layout";
import { DrawerHost } from "../drawer/DrawerHost";
import { openWorkspace, WorkspaceContext } from "../state/workspace";
import { createFakeApp } from "../testing/fakeApp";
import { Todos } from "./Todos";

export const USER: Actor = { kind: "user", id: "you", parent: null };

export const todo = (id: number, over: Partial<Todo> = {}): Todo => ({
  id,
  title: `todo ${id}`,
  body: "",
  done: false,
  blockers: [],
  blocked: false,
  created_at: 0,
  ...over,
});

export const todoEvent = (name: "todo.created" | "todo.updated", data: Todo): DaemonEvent => ({
  actor: USER,
  name,
  data,
});

/** The Shelf's Todos and the Drawer host over a fake Daemon whose `todo.list` answers `store.todos`. */
export const mountTodos = async (initial: Todo[]) => {
  const app = createFakeApp();
  const store = { todos: initial };
  app.handlers["rail.tree"] = () => [];
  app.handlers["terminal.list"] = () => [];
  app.handlers["provenance.history"] = () => [];
  app.handlers["todo.list"] = () => store.todos;
  app.handlers["todo.get"] = ({ id }) => store.todos.find((candidate) => candidate.id === id) ?? todo(id);

  const workspace = await openWorkspace(app, { name: "p", path: "/p" }, () => false);

  render(() => (
    <WorkspaceContext.Provider value={workspace}>
      <Layout
        header="roundup"
        rail={null}
        centre={null}
        shelf={<Todos />}
        overlay={<DrawerHost drawer={workspace.drawer} reducedMotion={() => true} />}
      />
    </WorkspaceContext.Provider>
  ));

  return { app, store, workspace };
};

export const callsTo = (app: ReturnType<typeof createFakeApp>, method: string) =>
  app.calls.filter((call) => call.method === method);
