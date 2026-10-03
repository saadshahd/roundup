import { createSignal } from "solid-js";
import type { Accessor } from "solid-js";
import type { OutputEvent } from "@contracts/terminal/OutputEvent";
import type { ConnectedProject } from "../state/connectedProject";
import { MAX_HELD_CHARS } from "../state/output";
import { fromBase64, toBase64 } from "./base64";
import type { Emulator, EmulatorFactory, Size } from "./emulator";

export type Screens = {
  /** The Terminal's emulator, created on first call; `terminal.output` arriving first creates it, so a pane shown later is not blank. */
  emulatorFor(id: string): Emulator;
  /** Opens a Terminal's emulator in the pane and records the fitted size for later snapshot recovery. */
  show(id: string, host: HTMLElement, focus?: boolean): Size;
  /** Tells the Daemon the size of the Terminal's pane. A Terminal that has exited, or a Daemon that has, takes no resize. */
  resize(id: string, size: Size): void;
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
  pendingChars: number;
  after: number | null;
  restoring: boolean;
  complete: boolean;
  lostOutput: boolean;
  shownSize: Size | null;
};

export const createScreens = (connected: ConnectedProject, createEmulator: EmulatorFactory): Screens => {
  const { app, output, rail, daemonExit } = connected;
  const holders = new Map<string, Holder>();
  const [failure, setFailure] = createSignal<string | null>(null);
  let disposed = false;

  const resize = (id: string, size: Size): void => {
    const holder = holders.get(id);

    if (holder?.shownSize) holder.shownSize = size;

    if (!exited(id) && daemonExit() === null) void attempt(app.rpc("terminal.resize", { id, ...size }));
  };

  const reflow = (id: string, holder: Holder): void => {
    if (disposed || !holder.shownSize) return;

    const fitted = holder.emulator.fit();

    if (fitted.cols !== holder.shownSize.cols || fitted.rows !== holder.shownSize.rows) resize(id, fitted);
  };

  const takeFirst = (holder: Holder): OutputEvent | undefined => {
    const chunk = holder.pending.shift();

    if (chunk) holder.pendingChars -= chunk.data.length;

    return chunk;
  };

  const drain = (id: string, holder: Holder): void => {
    if (holder.restoring || holder.after === null || disposed) return;

    while (holder.pending.length > 0) {
      const chunk = holder.pending[0];

      if (!chunk) return;

      try {
        const bytes = fromBase64(chunk.data);
        const end = chunk.offset + bytes.length;

        if (end <= holder.after) {
          takeFirst(holder);

          continue;
        }

        if (chunk.offset > holder.after) {
          void restore(id, holder);

          return;
        }

        holder.emulator.write(bytes.subarray(holder.after - chunk.offset));
        holder.after = end;
        takeFirst(holder);
      } catch (thrown) {
        if (!(thrown instanceof Error)) throw thrown;

        takeFirst(holder);
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
      holder.emulator.write(bytes, () => requestAnimationFrame(() => reflow(id, holder)));
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

    if (holder.lostOutput) {
      holder.lostOutput = false;
      void restore(id, holder);
    } else {
      drain(id, holder);
    }
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
      emulator, atBottom, returnToBottom, pending: [], pendingChars: 0,
      after: null, restoring: false, complete: false, lostOutput: false, shownSize: null,
    };

    holders.set(id, holder);
    void restore(id, holder);

    return holder;
  };

  const emulatorFor = (id: string): Emulator => holderFor(id).emulator;

  const show = (id: string, host: HTMLElement, focus?: boolean): Size => {
    const holder = holderFor(id);
    const size = holder.emulator.show(host, focus);

    holder.shownSize = size;

    return size;
  };

  const stopListening = output.subscribe((chunk) => {
    try {
      const holder = holderFor(chunk.id);

      holder.pending.push(chunk);
      holder.pendingChars += chunk.data.length;

      while (holder.pendingChars > MAX_HELD_CHARS) {
        takeFirst(holder);
        holder.lostOutput = true;
      }

      if (holder.lostOutput && !holder.restoring) {
        holder.lostOutput = false;
        void restore(chunk.id, holder);
      } else {
        drain(chunk.id, holder);
      }
    } catch (thrown) {
      if (!(thrown instanceof Error)) throw thrown;

      setFailure(`terminal.output: ${thrown.message}`);
    }
  });

  return {
    emulatorFor,
    show,
    resize,
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
