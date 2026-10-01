import { render } from "@solidjs/testing-library";
import type { Actor } from "@contracts/Actor";
import type { Touch } from "@contracts/Touch";
import type { Pad } from "@contracts/pad/Pad";
import { RpcError } from "../app/seam";
import { DrawerHost } from "../drawer/DrawerHost";
import { createFakeApp } from "../testing/fakeApp";
import { connectProject, ConnectedProjectContext } from "../state/connectedProject";
import { Pads } from "./Pads";

export const AGENT: Actor = { kind: "agent", id: "agent-7f3", parent: null };

const AGENT_NAME = "auth-refactor";

export const YOU: Actor = { kind: "user", id: "you", parent: null };

export const padOf = (name: string, owner: Actor, text = ""): Pad => ({ name, owner, text, updated_at: 0 });

export const CONFLICT = -32003;

export { RpcError };

export type Deferred<T> = { promise: Promise<T>; resolve(value: T): void; reject(error: Error): void };

export const deferred = <T,>(): Deferred<T> => {
  let resolve: (value: T) => void = () => {};

  let reject: (error: Error) => void = () => {};

  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });

  return { promise, resolve, reject };
};

/** The Shelf and the Drawer over a fake Daemon whose Pads are `state.pads`; `state.history` answers `provenance.history`. */
export const openShelf = async (pads: Pad[]) => {
  const history: Touch[] = [];
  const state = { pads, history };
  const app = createFakeApp();
  app.handlers["rail.tree"] = () => [
    { id: AGENT.id, kind: "agent", name: AGENT_NAME, parent: null, order: 0, status: null, meta: false, terminal_id: null },
  ];
  app.handlers["pad.list"] = () => state.pads.map((pad) => ({ ...pad }));
  app.handlers["pad.read"] = ({ name }) => {
    const found = state.pads.find((pad) => pad.name === name);

    if (!found) throw new RpcError(-32004, `no pad ${name}`);

    return { ...found };
  };

  app.handlers["provenance.history"] = () => state.history;

  const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, () => 0);

  render(() => (
    <ConnectedProjectContext.Provider value={connected}>
      <Pads />
      <DrawerHost drawer={connected.drawer} reducedMotion={() => true} />
    </ConnectedProjectContext.Provider>
  ));

  return { app, state, connected, calls: () => app.calls.map((call) => call.method) };
};
