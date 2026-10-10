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
  resize(id: string, size: Size, changedOnly?: boolean): void;
  setFontSize(size: number): void;
  /**
   * Whether the Terminal's view sits at its newest line; false once the user has scrolled up. Creates the
   * Terminal's emulator if none exists yet (true for that first read), so a caller never needs `emulatorFor`
   * called first for this to stay reactive.
   */
  isAtBottom(id: string): boolean;
  /** Returns the Terminal's view to its newest line, as clicking `↓ latest` or typing (U12) does. */
  returnToBottom(id: string): void;
  copy(id: string, clipboard: Pick<Clipboard, "writeText">): Promise<void>;
  paste(id: string, clipboard: Pick<Clipboard, "readText">): Promise<void>;
  /** Whether the Terminal has exited. */
  exited(id: string): boolean;
  /** Pastes dropped paths as one shell line fragment (U162); an exited Terminal or Daemon, or no path, takes nothing. */
  pastePaths(id: string, paths: string[]): void;
  /** The selected Terminal's last failed call or `terminal.output`, until a later call for that Terminal succeeds. */
  failure(id: string | null): string | null;
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

type FailureEntry = { message: string; order: number };

type TerminalFailures = { snapshot: FailureEntry | null; live: FailureEntry | null; clipboard: FailureEntry | null };

const newestFailure = (state: TerminalFailures | undefined): FailureEntry | null => {
  if (!state) return null;

  return [state.snapshot, state.live, state.clipboard].reduce<FailureEntry | null>(
    (latest, entry) => entry && (!latest || entry.order > latest.order) ? entry : latest,
    null,
  );
};

export const createScreens = (connected: ConnectedProject, createEmulator: EmulatorFactory): Screens => {
  const { app, output, rail, daemonExit } = connected;
  const holders = new Map<string, Holder>();
  const [failures, setFailures] = createSignal(new Map<string, TerminalFailures>());
  let failureOrder = 0;
  let disposed = false;

  const setFailure = (id: string, kind: keyof TerminalFailures, message: string | null): void => {
    setFailures((current) => {
      const previous = current.get(id) ?? { snapshot: null, live: null, clipboard: null };

      if (message === null && previous[kind] === null) return current;

      const state: TerminalFailures = {
        ...previous,
        [kind]: message === null ? null : { message, order: ++failureOrder },
      };

      const next = new Map(current);

      next.delete(id);

      if (state.snapshot !== null || state.live !== null || state.clipboard !== null) next.set(id, state);

      return next;
    });
  };

  const resize = (id: string, size: Size, changedOnly = false): void => {
    const holder = holders.get(id);

    if (changedOnly && holder?.shownSize?.cols === size.cols && holder.shownSize.rows === size.rows) return;

    if (holder?.shownSize) holder.shownSize = size;

    if (!exited(id) && daemonExit() === null) void attempt(id, app.rpc("terminal.resize", { id, ...size }));
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
        setFailure(id, "live", `terminal.output: ${thrown.message}`);
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
      setFailure(id, "snapshot", null);
    } catch (thrown) {
      if (!(thrown instanceof Error)) throw thrown;

      if (disposed) return;

      setFailure(id, "snapshot", `terminal.snapshot: ${thrown.message}`);

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

  const attempt = async (id: string, call: Promise<unknown>): Promise<void> => {
    try {
      await call;

      setFailure(id, "live", null);
    } catch (thrown) {
      if (!(thrown instanceof Error)) throw thrown;

      setFailure(id, "live", thrown.message);
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
          await attempt(id, app.rpc("terminal.write", { id, data }));
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

  const copy = async (id: string, clipboard: Pick<Clipboard, "writeText">): Promise<void> => {
    const selection = emulatorFor(id).selection();

    if (!selection) return;

    try {
      await clipboard.writeText(selection);
      setFailure(id, "clipboard", null);
    } catch (thrown) {
      if (!(thrown instanceof Error)) throw thrown;

      setFailure(id, "clipboard", thrown.message);
    }
  };

  const paste = async (id: string, clipboard: Pick<Clipboard, "readText">): Promise<void> => {
    if (exited(id) || daemonExit() !== null) return;

    try {
      const text = await clipboard.readText();

      if (text && !exited(id) && daemonExit() === null) emulatorFor(id).paste(text);

      setFailure(id, "clipboard", null);
    } catch (thrown) {
      if (!(thrown instanceof Error)) throw thrown;

      setFailure(id, "clipboard", thrown.message);
    }
  };

  const pastePaths = (id: string, paths: string[]): void => {
    if (paths.length === 0 || exited(id) || daemonExit() !== null) return;

    emulatorFor(id).paste(`${paths.map((path) => `'${path.replaceAll("'", "'\\''")}'`).join(" ")} `);
  };

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

      setFailure(chunk.id, "live", `terminal.output: ${thrown.message}`);
    }
  });

  return {
    emulatorFor,
    show,
    resize,
    setFontSize: (size) => {
      for (const holder of holders.values()) holder.emulator.setFontSize(size);
    },
    isAtBottom: (id) => holderFor(id).atBottom(),
    returnToBottom: (id) => holderFor(id).returnToBottom(),
    copy,
    paste,
    pastePaths,
    exited,
    failure: (id) => {
      const current = failures();

      if (id !== null) return newestFailure(current.get(id))?.message ?? null;

      let latest: FailureEntry | null = null;

      for (const state of current.values()) {
        const failure = newestFailure(state);

        if (failure && (!latest || failure.order > latest.order)) latest = failure;
      }

      return latest?.message ?? null;
    },
    dispose: () => {
      disposed = true;
      stopListening();

      for (const holder of holders.values()) holder.emulator.dispose();

      holders.clear();
    },
  };
};
