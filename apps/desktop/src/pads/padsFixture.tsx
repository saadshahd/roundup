import { render } from "@solidjs/testing-library";
import type { Actor } from "@contracts/Actor";
import type { Touch } from "@contracts/Touch";
import type { Pad } from "@contracts/pad/Pad";
import { RpcError } from "../app/seam";
import { DrawerHost } from "../drawer/DrawerHost";
import { createFakeApp } from "../testing/fakeApp";
import { openWorkspace, WorkspaceContext } from "../state/workspace";
import { Pads } from "./Pads";

export const AGENT: Actor = { kind: "agent", id: "auth-refactor", parent: null };

export const YOU: Actor = { kind: "user", id: "you", parent: null };

export const padOf = (name: string, owner: Actor, text = ""): Pad => ({ name, owner, text, updated_at: 0 });

export const CONFLICT = -32003;

export { RpcError };

/** The Shelf and the Drawer over a fake Daemon whose Pads are `state.pads`; `state.history` answers `provenance.history`. */
export const openShelf = async (pads: Pad[]) => {
  const history: Touch[] = [];
  const state = { pads, history };
  const app = createFakeApp();
  app.handlers["rail.tree"] = () => [];
  app.handlers["terminal.list"] = () => [];
  app.handlers["pad.list"] = () => state.pads.map((pad) => ({ ...pad }));
  app.handlers["pad.read"] = ({ name }) => {
    const found = state.pads.find((pad) => pad.name === name);

    if (!found) throw new RpcError(-32004, `no pad ${name}`);

    return { ...found };
  };

  app.handlers["provenance.history"] = () => state.history;

  const workspace = await openWorkspace(app, { name: "p", path: "/p" }, () => false);

  render(() => (
    <WorkspaceContext.Provider value={workspace}>
      <Pads />
      <DrawerHost drawer={workspace.drawer} reducedMotion={() => true} />
    </WorkspaceContext.Provider>
  ));

  return { app, state, workspace, calls: () => app.calls.map((call) => call.method) };
};
