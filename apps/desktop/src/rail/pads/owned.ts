import type { Actor } from "@contracts/Actor";
import type { RailNode } from "@contracts/agent/RailNode";
import type { Pad } from "@contracts/pad/Pad";
import type { Todo } from "@contracts/todo/Todo";

/** True when an Agent that owns `pad` is a row on the Rail (U58). A Pad of the user, an Extension, or a removed Agent is not. */
export const isOwnedOnRail = (pad: Pad, nodes: readonly RailNode[]): boolean =>
  pad.owner.kind === "agent" && nodes.some((node) => node.kind === "agent" && node.id === pad.owner.id);

/** The Pads `agent` owns, in `pad.list` order. */
export const padsOwnedBy = (pads: readonly Pad[], agent: Actor["id"]): Pad[] =>
  pads.filter((pad) => pad.owner.kind === "agent" && pad.owner.id === agent);

/** True when an Agent that created `todo` is a row on the Rail and the Todo is open (U60). A done Todo, one of the user's, an Extension's or a removed Agent's is not. */
export const isTodoOnRail = (todo: Todo, nodes: readonly RailNode[]): boolean =>
  !todo.done && todo.creator.kind === "agent" && nodes.some((node) => node.kind === "agent" && node.id === todo.creator.id);

/** The open Todos `agent` created, in id order. */
export const openTodosBy = (todos: readonly Todo[], agent: Actor["id"]): Todo[] =>
  todos.filter((todo) => !todo.done && todo.creator.kind === "agent" && todo.creator.id === agent).toSorted((a, b) => a.id - b.id);
