import type { Kind } from "@contracts/Kind";
import type { Todo } from "@contracts/todo/Todo";

const byId = (a: Todo, b: Todo): number => a.id - b.id;

export const kindOf = (todo: Todo): Kind => (todo.done ? "done" : todo.blocked ? "blocked" : "idle");

/** T10: the Daemon's order, never sorted here. */
export const openTodos = (all: readonly Todo[]): Todo[] => all.filter((todo) => !todo.done);

export const doneTodos = (all: readonly Todo[]): Todo[] => all.filter((todo) => todo.done);

/** Every blocker of `todo` still in the list, done or not, in id order. */
export const blockersOf = (todo: Todo, known: ReadonlyMap<number, Todo>): Todo[] =>
  todo.blockers.flatMap((id) => known.get(id) ?? []).toSorted(byId);

export const openBlockersOf = (todo: Todo, known: ReadonlyMap<number, Todo>): Todo[] =>
  blockersOf(todo, known).filter((blocker) => !blocker.done);

export const blockedBy = (todo: Todo, all: readonly Todo[]): Todo[] =>
  all.filter((other) => other.blockers.includes(todo.id)).toSorted(byId);

export const offeredAsBlockers = (todo: Todo, all: readonly Todo[]): Todo[] =>
  all.filter((other) => other.id !== todo.id && !todo.blockers.includes(other.id)).toSorted(byId);
