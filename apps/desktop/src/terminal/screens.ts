import { createSignal } from "solid-js";
import type { Accessor } from "solid-js";
import type { RailNode } from "@contracts/agent/RailNode";
import type { OutputEvent } from "@contracts/terminal/OutputEvent";
import type { ConnectedProject } from "../state/connectedProject";
import { fromBase64, toBase64 } from "./base64";
import type { Emulator, EmulatorFactory, Size } from "./emulator";

export type Screens = {
  /** The Terminal's emulator, created on first call; `terminal.output` arriving first creates it, so a pane shown later is not blank. */
  emulatorFor(id: string): Emulator;
  /** Tells the Daemon the size of the Terminal's pane. A Terminal that has exited, or a Daemon that has, takes no resize. */
  resize(id: string, size: Size): void;
  /** Stops the program behind the row: `agent.stop` for an Agent, `terminal.kill` for a Terminal. */
  stop(node: RailNode): void;
  /**
   * Whether the Terminal's view sits at its newest line; false once the user has scrolled up. Creates the
   * Terminal's emulator if none exists yet (true for that first read), so a caller never needs `emulatorFor`
   * called first for this to stay reactive.
   */
  isAtBottom(id: string): boolean;
  /** Returns the Terminal's view to its newest line, as clicking `↓ latest` or typing (U12) does. */
  returnToBottom(id: string): void;
  /** The last call or `terminal.output` that failed, until a later call succeeds. */
  failure: Accessor<string | null>;
  dispose(): void;
};

type Holder = {
  emulator: Emulator;
  atBottom: Accessor<boolean>;
  returnToBottom: () => void;
  pending: OutputEvent[];
  after: number | null;
  restoring: boolean;
  complete: boolean;
};

export const createScreens = (connected: ConnectedProject, createEmulator: EmulatorFactory): Screens => {
  const { app, output, rail, daemonExit } = connected;
  const holders = new Map<string, Holder>();
  const [failure, setFailure] = createSignal<string | null>(null);
  let disposed = false;

  const drain = (id: string, holder: Holder): void => {
    if (holder.restoring || holder.after === null || disposed) return;

    while (holder.pending.length > 0) {
      const chunk = holder.pending[0];

      if (!chunk) return;

      try {
        const bytes = fromBase64(chunk.data);
        const end = chunk.offset + bytes.length;

        if (end <= holder.after) {
          holder.pending.shift();

          continue;
        }

        if (chunk.offset > holder.after) {
          void restore(id, holder);

          return;
        }

        holder.emulator.write(bytes.subarray(holder.after - chunk.offset));
        holder.after = end;
        holder.pending.shift();
      } catch (thrown) {
        if (!(thrown instanceof Error)) throw thrown;

        holder.pending.shift();
        setFailure(`terminal.output: ${thrown.message}`);
      }
    }
  };

  const restore = async (id: string, holder: Holder): Promise<void> => {
    if (holder.restoring || disposed) return;

    holder.restoring = true;

    try {
      const snapshot = await app.rpc("terminal.snapshot", { id });

      if (disposed) return;

      const bytes = fromBase64(snapshot.data);

      if (holder.complete) holder.emulator.reset();

      holder.emulator.setSize({ cols: snapshot.cols, rows: snapshot.rows });
      holder.emulator.write(bytes);
      holder.after = snapshot.after;
      holder.complete = true;
      setFailure(null);
    } catch (thrown) {
      if (!(thrown instanceof Error)) throw thrown;

      if (disposed) return;

      setFailure(`terminal.snapshot: ${thrown.message}`);

      if (!holder.complete) holder.after = holder.pending[0]?.offset ?? 0;

      holder.restoring = false;

      if (!holder.complete) drain(id, holder);

      return;
    }

    holder.restoring = false;
    drain(id, holder);
  };

  const exited = (id: string): boolean => {
    const node = rail.nodes.find((candidate) => candidate.terminal_id === id);

    return node !== undefined && rail.exitOf(node) !== null;
  };

  const attempt = async (call: Promise<unknown>): Promise<void> => {
    try {
      await call;
      setFailure(null);
    } catch (thrown) {
      if (!(thrown instanceof Error)) throw thrown;

      setFailure(thrown.message);
    }
  };

  const holderFor = (id: string): Holder => {
    const known = holders.get(id);

    if (known) return known;

    const emulator = createEmulator(id);
    const [atBottom, setAtBottom] = createSignal(true);
    // Tauri may run two `rpc` calls out of order, so at most one write is in flight; what is typed meanwhile goes as one write.
    let typed: number[] = [];
    let writing = false;

    const flush = async (): Promise<void> => {
      writing = true;

      try {
        while (typed.length > 0) {
          const data = toBase64(Uint8Array.from(typed));

          typed = [];
          await attempt(app.rpc("terminal.write", { id, data }));
        }
      } finally {
        writing = false;
      }
    };

    const returnToBottom = () => {
      emulator.scrollToBottom();
      setAtBottom(true);
    };

    emulator.onScroll(() => setAtBottom(emulator.isAtBottom()));
    emulator.onInput((bytes) => {
      if (exited(id) || daemonExit() !== null) return;

      returnToBottom();

      for (const byte of bytes) typed.push(byte);

      if (!writing) void flush();
    });
    const holder: Holder = {
      emulator, atBottom, returnToBottom, pending: [], after: null, restoring: false, complete: false,
    };

    holders.set(id, holder);
    void restore(id, holder);

    return holder;
  };

  const emulatorFor = (id: string): Emulator => holderFor(id).emulator;

  const stopListening = output.subscribe((chunk) => {
    try {
      const holder = holderFor(chunk.id);

      holder.pending.push(chunk);
      drain(chunk.id, holder);
    } catch (thrown) {
      if (!(thrown instanceof Error)) throw thrown;

      setFailure(`terminal.output: ${thrown.message}`);
    }
  });

  return {
    emulatorFor,
    resize: (id, size) => {
      if (!exited(id) && daemonExit() === null) void attempt(app.rpc("terminal.resize", { id, ...size }));
    },
    stop: (node) => {
      if (node.status !== null) {
        void attempt(app.rpc("agent.stop", { id: node.id }));
      } else if (node.terminal_id !== null) {
        void attempt(app.rpc("terminal.kill", { id: node.terminal_id }));
      }
    },
    isAtBottom: (id) => holderFor(id).atBottom(),
    returnToBottom: (id) => holderFor(id).returnToBottom(),
    failure,
    dispose: () => {
      disposed = true;
      stopListening();

      for (const holder of holders.values()) holder.emulator.dispose();

      holders.clear();
    },
  };
};
