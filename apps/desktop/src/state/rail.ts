import { createSignal } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import type { Actor } from "@contracts/Actor";
import type { Event as DaemonEvent } from "@contracts/Event";
import type { RailNode } from "@contracts/agent/RailNode";
import type { Events } from "../app/events";
import type { AppSeam } from "../app/seam";

/** How an exited node's program ended: with a code, by a signal, or unknown because the node has no Terminal. */
export type ExitState = { kind: "code"; code: number } | { kind: "signal" } | { kind: "unknown" };

export type RailState = {
  /** Rows in the Daemon's order; only a `rail.tree` call adds or removes one. */
  readonly nodes: readonly RailNode[];
  /** `null` while the node's program runs, and for a plain Group (one with no program). */
  exitOf(node: RailNode): ExitState | null;
  selected(): string | null;
  select(id: string | null): void;
  nameOf(actor: Actor): string;
  /** The message of the last failed fetch, until a later fetch succeeds. */
  failure(): string | null;
  /** Resolves when no fetch is in flight and no queued Event is left to apply. */
  settled(): Promise<void>;
};

export const createRailState = (app: AppSeam, events: Events): RailState => {
  const [model, setModel] = createStore<{ tree: RailNode[]; exited: Record<string, number | null> }>({
    tree: [],
    exited: {},
  });

  const [selected, setSelected] = createSignal<string | null>(null);
  const [failure, setFailure] = createSignal<string | null>(null);

  let pending = 0;
  let queued: DaemonEvent[] = [];
  let latest: Promise<void> = Promise.resolve();

  const fetchTree = async () => {
    setModel("tree", reconcile(await app.rpc("rail.tree", null), { key: "id" }));

    const id = selected();

    if (id !== null && !model.tree.some((node) => node.id === id)) setSelected(null);
  };

  const fetchTerminals = async () => {
    for (const terminal of await app.rpc("terminal.list", null)) {
      if (!terminal.running) setModel("exited", terminal.id, terminal.exit_code);
    }
  };

  /** Fetches run one after another, and Events that arrive meanwhile wait: the answer may predate them. */
  const fetching = (work: () => Promise<void>): void => {
    pending += 1;

    latest = latest.then(work).then(
      () => {
        setFailure(null);
      },
      (error) => {
        if (!(error instanceof Error)) throw error;

        setFailure(error.message);
      },
    ).finally(() => {
      pending -= 1;

      if (pending === 0) apply(queued.splice(0));
    });
  };

  const apply = (batch: DaemonEvent[]): void => {
    for (const event of batch) {
      if (pending > 0) {
        queued.push(event);
      } else if (event.name === "rail.changed") {
        fetching(fetchTree);
      } else if (event.name === "agent.status") {
        setModel("tree", (node) => node.id === event.data.id, "status", event.data.status);
      } else if (event.name === "terminal.exited") {
        setModel("exited", event.data.id, event.data.code);
      }
    }
  };

  events.subscribe((event) => apply([event]));
  fetching(async () => {
    await Promise.all([fetchTree(), fetchTerminals()]);
  });

  return {
    get nodes() {
      return model.tree;
    },
    exitOf: (node) => {
      if (node.kind === "group" && !node.meta) return null;

      if (node.terminal_id === null) return { kind: "unknown" };

      if (!(node.terminal_id in model.exited)) return null;

      const code = model.exited[node.terminal_id] ?? null;

      return code === null ? { kind: "signal" } : { kind: "code", code };
    },
    failure,
    selected,
    select: (id) => {
      setSelected(id !== null && model.tree.some((node) => node.id === id) ? id : null);
    },
    nameOf: (actor) =>
      actor.kind === "user"
        ? "you"
        : actor.kind === "agent"
          ? (model.tree.find((node) => node.id === actor.id)?.name ?? actor.id)
          : actor.id,
    settled: async () => {
      let seen: Promise<void>;

      do {
        seen = latest;
        await seen;
      } while (latest !== seen);
    },
  };
};
