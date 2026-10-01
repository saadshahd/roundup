import { createSignal } from "solid-js";
import type { Accessor } from "solid-js";
import type { RailNode } from "@contracts/agent/RailNode";
import type { ConnectedProject } from "../state/connectedProject";
import { fromBase64, toBase64 } from "./base64";
import type { Emulator, EmulatorFactory, Size } from "./emulator";

export type Screens = {
  /** The Terminal's emulator, created on first call; `terminal.output` arriving first creates it, so a pane shown later is not blank. */
  emulatorFor(id: string): Emulator;
  /** Tells the Daemon the size of the Terminal's pane. A Terminal that has exited takes no resize. */
  resize(id: string, size: Size): void;
  /** Stops the program behind the row: `agent.stop` for an Agent, `terminal.kill` for a Terminal. */
  stop(node: RailNode): void;
  /** The last call or `terminal.output` that failed, until a later call succeeds. */
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

    emulator.onInput((bytes) => {
      if (exited(id)) return;

      for (const byte of bytes) typed.push(byte);

      if (!writing) void flush();
    });
    emulators.set(id, emulator);

    return emulator;
  };

  const stopListening = output.subscribe((chunk) => {
    // A throw escapes into the Tauri Channel callback, which then never advances its message index, so every later
    // Daemon Event is held forever; during the feed's replay it would also abort the Pane's mount.
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
    stop: (node) => {
      if (node.status !== null) {
        void attempt(app.rpc("agent.stop", { id: node.id }));
      } else if (node.terminal_id !== null) {
        void attempt(app.rpc("terminal.kill", { id: node.terminal_id }));
      }
    },
    failure,
    dispose: () => {
      stopListening();

      for (const emulator of emulators.values()) emulator.dispose();

      emulators.clear();
    },
  };
};
