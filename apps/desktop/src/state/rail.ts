import { batch, createSignal } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import type { Actor } from "@contracts/Actor";
import type { Event as DaemonEvent } from "@contracts/Event";
import type { RailNode } from "@contracts/agent/RailNode";
import { ancestorsOf, isRoom } from "../rail/layout";
import type { RailStorage } from "../rail/persist/storage";
import type { Events } from "../app/events";
import type { AppSeam } from "../app/seam";

/** How an exited node's program ended: with a code, by a signal, or unknown because the node has no Terminal. */
export type ExitState = { kind: "code"; code: number } | { kind: "signal" } | { kind: "unknown" };

export type RailState = {
  /** Rows in the Daemon's order; only a `rail.tree` call adds or removes one. */
  readonly nodes: readonly RailNode[];
  /** `null` while the node's program runs, and for a Room (one with no program). */
  exitOf(node: RailNode): ExitState | null;
  doorPending(id: string): boolean;
  doorFailure(id: string): string | null;
  startDoor(id: string): Promise<void>;
  /** U9/U62: creates a root Room, then selects it and starts its Door once `rail.tree` returns it. */
  createRoom(): Promise<void>;
  /** True from the click until the new Room has appeared and its Door launch has begun. */
  roomCreating(): boolean;
  /** The message of the last failed `rail.createRoom`, until the next attempt or click. */
  roomFailure(): string | null;
  clearRoomFailure(): void;
  selected(): string | null;
  restored(): boolean;
  collapsed(): ReadonlySet<string>;
  toggleCollapsed(id: string): void;
  storageFailure(): string | null;
  clearStorageFailure(): void;
  select(id: string | null): void;
  nameOf(actor: Actor): string;
  /** The message of the last failed fetch, until a later fetch succeeds. */
  failure(): string | null;
  /** Fetches the current tree after a write found that its row had already gone; `terminals` also refetches `terminal.list`, so a missed exit shows. */
  refresh(options?: { terminals?: boolean }): Promise<void>;
  /** Resolves when no fetch is in flight and no queued Event is left to apply. */
  settled(): Promise<void>;
};

export const createRailState = (app: AppSeam, events: Events, storage?: RailStorage): RailState => {
  const [model, setModel] = createStore<{ tree: RailNode[]; exited: Record<string, number | null> }>({
    tree: [],
    exited: {},
  });

  const [doors, setDoors] = createStore<Record<string, { pending: boolean; observedLive: boolean; failure: { message: string; attempt: string | null } | null }>>({});
  const [selected, setSelected] = createSignal<string | null>(null);
  const [failure, setFailure] = createSignal<string | null>(null);
  const [roomCreating, setRoomCreating] = createSignal(false);
  const [roomFailure, setRoomFailure] = createSignal<string | null>(null);
  /** The id `rail.createRoom` returned, until `rail.tree` shows its row. */
  let wantedRoom: string | null = null;

  const [collapsed, setCollapsed] = createSignal<ReadonlySet<string>>(new Set());
  const [restored, setRestored] = createSignal(false);
  const [storageFailure, setStorageFailure] = createSignal<string | null>(null);
  let firstTree = true;

  const save = () => setStorageFailure(storage?.write({ selected: selected(), collapsed: [...collapsed()] }) ?? null);

  const reveal = (id: string | null) => {
    if (id === null) return;

    const above = new Set(ancestorsOf(model.tree, id));

    setCollapsed((closed) => new Set([...closed].filter((room) => !above.has(room))));
  };

  let pending = 0;
  let queued: DaemonEvent[] = [];
  let latest: Promise<void> = Promise.resolve();

  const select = (id: string | null): void => {
    batch(() => {
      setRestored(false);
      setSelected(id !== null && model.tree.some((node) => node.id === id) ? id : null);
      reveal(selected());
      save();
    });
  };

  const hasLiveTerminal = (node: RailNode) => node.terminal_id !== null && !(node.terminal_id in model.exited);

  const fetchTree = async () => {
    const tree = await app.rpc("rail.tree", null);

    batch(() => {
      setModel("tree", reconcile(tree, { key: "id" }));

      for (const node of tree) {
        if (doors[node.id]?.pending && hasLiveTerminal(node)) setDoors(node.id, "observedLive", true);
        const failed = doors[node.id]?.failure;

        if (failed && (hasLiveTerminal(node) || (node.attempt !== null && BigInt(node.attempt) > BigInt(failed.attempt ?? "0")))) {
          setDoors(node.id, "failure", null);
        }
      }

      if (firstTree) {
        firstTree = false;
        const saved = storage?.read();

        if (saved) {
          setSelected(tree.some((node) => node.id === saved.selected) ? saved.selected : null);
          setCollapsed(new Set(saved.collapsed.filter((id) => tree.some((node) => node.id === id && isRoom(node)))));
          reveal(selected());
          setRestored(selected() !== null);
          save();
        }
      } else {
        const id = selected();

        if (id !== null && !tree.some((node) => node.id === id)) {
          setSelected(null);
          save();
        }
      }
    });

    const wanted = wantedRoom;

    if (wanted !== null && tree.some((node) => node.id === wanted)) {
      wantedRoom = null;
      select(wanted);
      setRoomCreating(false);
      void startDoor(wanted);
    }
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
        setModel("tree", (node) => node.id === event.data.id && node.attempt === event.data.attempt && node.status_revision !== null && BigInt(event.data.status_revision) > BigInt(node.status_revision), { status: event.data.status, status_revision: event.data.status_revision });
      } else if (event.name === "terminal.exited") {
        setModel("exited", event.data.id, event.data.code);
      }
    }
  };

  const startDoor = async (id: string): Promise<void> => {
    if (doors[id]?.pending) return;
    setDoors(id, { pending: true, observedLive: false, failure: null });

    try {
      await app.rpc("rail.startDoor", { id });
      fetching(fetchTree);
      await latest;
    } catch (error) {
      if (!(error instanceof Error)) throw error;
      fetching(fetchTree);
      await latest;
      const node = model.tree.find((each) => each.id === id);

      if (node && !hasLiveTerminal(node) && !doors[id]?.observedLive) {
        setDoors(id, "failure", { message: error.message, attempt: node.attempt });
      }
    } finally {
      setDoors(id, "pending", false);
    }
  };

  const createRoom = async (): Promise<void> => {
    if (roomCreating()) return;
    setRoomCreating(true);
    setRoomFailure(null);

    try {
      wantedRoom = (await app.rpc("rail.createRoom", { name: "room", parent: null })).id;
    } catch (error) {
      if (!(error instanceof Error)) throw error;

      setRoomFailure(error.message);
      setRoomCreating(false);

      return;
    }

    fetching(fetchTree);
  };

  events.subscribe((event) => apply([event]));
  fetching(async () => {
    await Promise.all([fetchTree(), fetchTerminals()]);
  });

  return {
    doorPending: (id) => doors[id]?.pending ?? false,
    doorFailure: (id) => doors[id]?.failure?.message ?? null,
    startDoor,
    createRoom,
    roomCreating,
    roomFailure,
    clearRoomFailure: () => setRoomFailure(null),
    get nodes() {
      return model.tree;
    },
    exitOf: (node) => {
      if (node.terminal_id === null) return { kind: "unknown" };

      if (!(node.terminal_id in model.exited)) return null;

      const code = model.exited[node.terminal_id] ?? null;

      return code === null ? { kind: "signal" } : { kind: "code", code };
    },
    failure,
    refresh: async (options) => {
      if (options?.terminals) fetching(async () => { await Promise.all([fetchTree(), fetchTerminals()]); });
      else fetching(fetchTree);
      await latest;
    },
    selected,
    restored,
    collapsed,
    storageFailure,
    clearStorageFailure: () => setStorageFailure(null),
    toggleCollapsed: (id) => {
      setCollapsed((closed) => new Set(closed.has(id) ? [...closed].filter((each) => each !== id) : [...closed, id]));
      save();
    },
    select,
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
