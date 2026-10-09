import { createRoot, onCleanup } from "solid-js";
import { useConnectedProject } from "../state/connectedProject";
import type { ConnectedProject } from "../state/connectedProject";
import { createTodosState } from "./state";
import type { TodosState } from "./state";

type Shared = { list: TodosState; users: number; dispose: () => void };

const shared = new WeakMap<ConnectedProject, Shared>();

const open = (connected: ConnectedProject): Shared =>
  createRoot((dispose) => ({ list: createTodosState(connected.app, connected.events), users: 0, dispose }));

/** One `todo.list`, refetched as U15 says, serves the Shelf, the Drawer and every Rail row (U60); it logs no Touch. The fetch and the subscription end with the last reader. */
export const useTodoList = (): TodosState => {
  const connected = useConnectedProject();
  const entry = shared.get(connected) ?? open(connected);

  shared.set(connected, entry);
  entry.users += 1;

  onCleanup(() => {
    entry.users -= 1;

    if (entry.users > 0) return;

    shared.delete(connected);
    entry.dispose();
  });

  return entry.list;
};
