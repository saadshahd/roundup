import type { Kind } from "@contracts/Kind";
import type { Todo } from "@contracts/todo/Todo";

const byId = (a: Todo, b: Todo): number => a.id - b.id;

export const kindOf = (todo: Todo): Kind => (todo.done ? "done" : todo.blocked ? "blocked" : "idle");

export const openTodos = (all: readonly Todo[]): Todo[] => all.filter((todo) => !todo.done).toSorted(byId);

export const doneTodos = (all: readonly Todo[]): Todo[] => all.filter((todo) => todo.done).toSorted(byId);

/** Every blocker of `todo` still in the list, done or not, in id order. */
export const blockersOf = (todo: Todo, all: readonly Todo[]): Todo[] =>
  all.filter((other) => todo.blockers.includes(other.id)).toSorted(byId);

export const openBlockersOf = (todo: Todo, all: readonly Todo[]): Todo[] =>
  blockersOf(todo, all).filter((blocker) => !blocker.done);

export const blockedBy = (todo: Todo, all: readonly Todo[]): Todo[] =>
  all.filter((other) => other.blockers.includes(todo.id)).toSorted(byId);

export const offeredAsBlockers = (todo: Todo, all: readonly Todo[]): Todo[] =>
  all.filter((other) => other.id !== todo.id && !todo.blockers.includes(other.id)).toSorted(byId);
