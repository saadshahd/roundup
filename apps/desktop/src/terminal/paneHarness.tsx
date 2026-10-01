import { render } from "@solidjs/testing-library";
import type { Actor } from "@contracts/Actor";
import type { Event as DaemonEvent } from "@contracts/Event";
import type { EventData } from "@contracts/EventData";
import type { RailNode } from "@contracts/agent/RailNode";
import type { TerminalInfo } from "@contracts/terminal/TerminalInfo";
import { createFakeApp } from "../testing/fakeApp";
import type { FakeApp } from "../testing/fakeApp";
import { connectProject, ConnectedProjectContext } from "../state/connectedProject";
import type { ConnectedProject } from "../state/connectedProject";
import { Pane } from "./Pane";
import type { Emulator, EmulatorFactory, Size } from "./emulator";
import { toBase64 } from "./base64";

const USER: Actor = { kind: "user", id: "you", parent: null };

type FakeEmulator = Emulator & {
  written: Uint8Array[];
  host: HTMLElement | null;
  /** What `show` and `fit` answer. */
  size: Size;
  fits: number;
  disposed: boolean;
  type(bytes: Uint8Array): void;
};

const fakeEmulators = () => {
  const made = new Map<string, FakeEmulator>();

  const factory: EmulatorFactory = (id) => {
    let listener: (bytes: Uint8Array) => void = () => {};

    const emulator: FakeEmulator = {
      written: [],
      host: null,
      size: { cols: 100, rows: 30 },
      fits: 0,
      disposed: false,
      write: (bytes) => {
        emulator.written.push(bytes);
      },
      onInput: (next) => {
        listener = next;
      },
      show: (host) => {
        emulator.host = host;
        host.replaceChildren(id);

        return emulator.size;
      },
      fit: () => {
        emulator.fits += 1;

        return emulator.size;
      },
      dispose: () => {
        emulator.disposed = true;
      },
      type: (bytes) => listener(bytes),
    };

    made.set(id, emulator);

    return emulator;
  };

  return { factory, made };
};

export const node = (id: string, over: Partial<RailNode> = {}): RailNode => ({
  id,
  kind: "agent",
  name: id,
  parent: null,
  order: 0,
  status: { kind: "working", label: "starting", since: 0 },
  meta: false,
  terminal_id: `t-${id}`,
  ...over,
});

export const shell = (id: string, over: Partial<RailNode> = {}): RailNode =>
  node(id, { kind: "terminal", status: null, ...over });

export const info = (id: string, over: Partial<TerminalInfo> = {}): TerminalInfo => ({
  id,
  cwd: "/p",
  title: null,
  running: true,
  exit_code: null,
  ...over,
});

export const output = (id: string, text: string): DaemonEvent => ({
  actor: USER,
  name: "terminal.output",
  data: { id, data: toBase64(new TextEncoder().encode(text)) },
});

export const event = (data: EventData): DaemonEvent => ({ actor: USER, ...data });

type Mounted = {
  app: FakeApp;
  connected: ConnectedProject;
  emulators: ReturnType<typeof fakeEmulators>["made"];
  container: HTMLElement;
};

/** A Pane inside an open Project whose Rail holds `tree` and whose Daemon lists `terminals`. */
export const mountPane = async (tree: RailNode[], terminals: TerminalInfo[] = [], now = 0): Promise<Mounted> => {
  const app = createFakeApp();

  app.handlers["rail.tree"] = () => tree;
  app.handlers["terminal.list"] = () => terminals;
  app.handlers["terminal.write"] = () => null;
  app.handlers["terminal.resize"] = () => null;

  const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, () => now);
  const { factory, made } = fakeEmulators();

  const { container } = render(() => (
    <ConnectedProjectContext.Provider value={connected}>
      <Pane createEmulator={factory} />
    </ConnectedProjectContext.Provider>
  ));

  return { app, connected, emulators: made, container };
};

export const callsTo = (app: FakeApp, method: "terminal.write" | "terminal.resize") =>
  app.calls.filter((call) => call.method === method).map((call) => call.params);
