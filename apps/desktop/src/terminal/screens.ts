import { createSignal } from "solid-js";
import type { Accessor } from "solid-js";
import type { ConnectedProject } from "../state/connectedProject";
import { fromBase64, toBase64 } from "./base64";
import type { Emulator, EmulatorFactory, Size } from "./emulator";

export type Screens = {
  /** The Terminal's emulator, created on first call; `terminal.output` arriving first creates it, so a pane shown later is not blank. */
  emulatorFor(id: string): Emulator;
  /** Tells the Daemon the size of the Terminal's pane. A Terminal that has exited takes no resize. */
  resize(id: string, size: Size): void;
  /** The last call or `terminal.output` that failed, until a later one succeeds. */
  failure: Accessor<string | null>;
  dispose(): void;
};

export const createScreens = (connected: ConnectedProject, createEmulator: EmulatorFactory): Screens => {
  const { app, output, rail } = connected;
  const emulators = new Map<string, Emulator>();
  const [failure, setFailure] = createSignal<string | null>(null);

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

  const emulatorFor = (id: string): Emulator => {
    const known = emulators.get(id);

    if (known) return known;

    const emulator = createEmulator(id);
    // Tauri may run two `rpc` calls out of order; each write waits for the one typed before it.
    let sent: Promise<void> = Promise.resolve();

    emulator.onInput((bytes) => {
      if (exited(id)) return;

      sent = sent.then(() => attempt(app.rpc("terminal.write", { id, data: toBase64(bytes) })));
    });
    emulators.set(id, emulator);

    return emulator;
  };

  const stopListening = output.subscribe((chunk) => {
    // A throw here would stop `connectEvents` from reaching the Rail state for the same Event.
    try {
      emulatorFor(chunk.id).write(fromBase64(chunk.data));
    } catch (thrown) {
      if (!(thrown instanceof Error)) throw thrown;

      setFailure(`terminal.output: ${thrown.message}`);
    }
  });

  return {
    emulatorFor,
    resize: (id, size) => {
      if (!exited(id)) void attempt(app.rpc("terminal.resize", { id, ...size }));
    },
    failure,
    dispose: () => {
      stopListening();

      for (const emulator of emulators.values()) emulator.dispose();

      emulators.clear();
    },
  };
};
