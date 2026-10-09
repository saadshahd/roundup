import type { EventData } from "@contracts/EventData";
import type { Touch } from "@contracts/Touch";
import type { Pad } from "@contracts/pad/Pad";
import type { Todo } from "@contracts/todo/Todo";
import { RpcError } from "../app/seam";
import type { Handlers } from "./fakeApp";
import { USER } from "./nodes";

const NOT_FOUND = -32001;

const INVALID_PARAMS = -32602;

const CONFLICT = -32003;

/** Where a handler reports the Event the real Daemon would send after the same write. */
export type Announce = (data: EventData) => void;

export type TodoStore = { todos: Todo[] };

export type PadStore = { pads: Pad[]; history: Touch[] };

const find = <T,>(items: T[], matches: (item: T) => boolean, what: string): T => {
  const found = items.find(matches);

  if (!found) throw new RpcError(NOT_FOUND, `not found: ${what}`);

  return found;
};

const withBlocked = (todos: Todo[]): Todo[] =>
  todos.map((todo) => ({
    ...todo,
    blocked: todo.blockers.some((id) => todos.some((other) => other.id === id && !other.done)),
  }));

/** Replaces `store.todos` with `change` applied, derives `blocked`, and announces `todo.updated` for `id` and `todo.unblocked` for every Todo the change freed. */
const changeTodos = (store: TodoStore, announce: Announce, id: number, change: (todos: Todo[]) => Todo[]): Todo => {
  const before = store.todos;
  store.todos = withBlocked(change(before));
  const updated = find(store.todos, (todo) => todo.id === id, `todo ${id}`);
  announce({ name: "todo.updated", data: updated });

  for (const todo of store.todos) {
    if (before.some((was) => was.id === todo.id && was.blocked) && !todo.blocked) announce({ name: "todo.unblocked", data: { id: todo.id } });
  }

  return { ...updated };
};

/** The Todo methods over `store.todos`; each write also sends the Events the real Daemon sends. */
export const todoHandlers = (store: TodoStore, announce: Announce): Handlers => ({
  "todo.list": () => store.todos.map((todo) => ({ ...todo })),
  "todo.get": ({ id }) => ({ ...find(store.todos, (todo) => todo.id === id, `todo ${id}`) }),
  "todo.create": ({ title, body, blockers }) => {
    const id = Math.max(0, ...store.todos.map((todo) => todo.id)) + 1;
    const created: Todo = { id, title, body: body ?? "", done: false, blockers: blockers ?? [], blocked: false, created_at: Date.now(), creator: USER };
    store.todos = withBlocked([...store.todos, created]);
    const stored = find(store.todos, (todo) => todo.id === id, `todo ${id}`);
    announce({ name: "todo.created", data: stored });

    return { ...stored };
  },
  "todo.update": ({ id, title, body }) =>
    changeTodos(store, announce, id, (todos) =>
      todos.map((todo) => (todo.id === id ? { ...todo, title: title ?? todo.title, body: body ?? todo.body } : todo)),
    ),
  "todo.complete": ({ id }) =>
    changeTodos(store, announce, id, (todos) => todos.map((todo) => (todo.id === id ? { ...todo, done: true } : todo))),
  "todo.setBlockers": ({ id, blockers }) =>
    changeTodos(store, announce, id, (todos) => todos.map((todo) => (todo.id === id ? { ...todo, blockers } : todo))),
  "todo.delete": ({ id }) => {
    find(store.todos, (todo) => todo.id === id, `todo ${id}`);
    store.todos = withBlocked(store.todos.filter((todo) => todo.id !== id).map((todo) => ({ ...todo, blockers: todo.blockers.filter((blocker) => blocker !== id) })));
    announce({ name: "todo.deleted", data: { id } });

    return null;
  },
});

const changePad = (store: PadStore, announce: Announce, name: string, change: (pad: Pad) => Pad): Pad => {
  const next = change(find(store.pads, (pad) => pad.name === name, `pad ${name}`));
  store.pads = store.pads.map((pad) => (pad.name === name ? next : pad));
  announce({ name: "pad.changed", data: { name } });

  return { ...next };
};

/** The Pad methods over `store.pads`, with the Provenance history `store.history`; each write also sends `pad.changed`. */
export const padHandlers = (store: PadStore, announce: Announce): Handlers => ({
  "pad.list": () => store.pads.map((pad) => ({ ...pad })),
  "pad.read": ({ name }) => ({ ...find(store.pads, (pad) => pad.name === name, `pad ${name}`) }),
  "pad.create": ({ name, text }) => {
    if (name.trim() === "") throw new RpcError(INVALID_PARAMS, "a Pad needs a name");

    if (store.pads.some((pad) => pad.name === name)) throw new RpcError(CONFLICT, `pad ${name} already exists`);

    const created: Pad = { name, owner: USER, text: text ?? "", updated_at: Date.now() };
    store.pads = [...store.pads, created];
    announce({ name: "pad.changed", data: { name } });

    return { ...created };
  },
  "pad.write": ({ name, text }) => changePad(store, announce, name, (pad) => ({ ...pad, text, updated_at: Date.now() })),
  "pad.append": ({ name, text }) => changePad(store, announce, name, (pad) => ({ ...pad, text: pad.text + text, updated_at: Date.now() })),
  "pad.setOwner": ({ name, owner }) => changePad(store, announce, name, (pad) => ({ ...pad, owner })),
  "pad.export": () => null,
  "provenance.history": () => store.history,
});
