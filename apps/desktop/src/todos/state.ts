import { createSignal, onCleanup } from "solid-js";
import type { Accessor } from "solid-js";
import type { Todo } from "@contracts/todo/Todo";
import type { Events } from "../app/events";
import type { AppSeam } from "../app/seam";
import { failureOf } from "./failureOf";

export type TodosState = {
  /** The Project's Todos as `todo.list` last answered. */
  all: Accessor<readonly Todo[]>;
  /** The message of the last failed `todo.list`, until a later one succeeds. */
  failure: Accessor<string | null>;
};

const REFETCH_ON = new Set(["todo.created", "todo.updated", "todo.unblocked", "todo.deleted"]);

/** Subscribes before its first fetch, so no Event between the two is lost. */
export const createTodosState = (app: AppSeam, events: Events): TodosState => {
  const [all, setAll] = createSignal<readonly Todo[]>([]);
  const [failure, setFailure] = createSignal<string | null>(null);

  // Overlapping fetches can answer out of order; only the newest one may write.
  let newest = 0;

  const refetch = async () => {
    newest += 1;

    const mine = newest;
    let list: readonly Todo[] = [];

    const message = await failureOf(async () => {
      list = await app.rpc("todo.list", null);
    });

    if (mine !== newest) return;

    if (message === null) setAll(list);

    setFailure(message);
  };

  onCleanup(
    events.subscribe((event) => {
      if (REFETCH_ON.has(event.name)) void refetch();
    }),
  );
  void refetch();

  return { all, failure };
};
