import { render } from "@solidjs/testing-library";
import { createSignal, Show } from "solid-js";
import type { RailNode } from "@contracts/agent/RailNode";
import { DrawerHost } from "../drawer/DrawerHost";
import { Rail } from "../rail/Rail";
import { ConnectedProjectContext, connectProject } from "../state/connectedProject";
import { createFakeApp } from "../testing/fakeApp";
import { NOW } from "../testing/nodes";
import { Pane } from "../terminal/Pane";
import type { EmulatorFactory } from "../terminal/emulator";
import { Keys } from "./Keys";

/** What else to mount alongside the Rail and `Keys`: a Pane (an `EmulatorFactory` gives it a focusable screen) or the DrawerHost. */
type KeysNeighbors = { pane?: true | EmulatorFactory; drawer?: boolean };

/** The Rail and `Keys` on a fake Daemon whose tree is `tree`, as `railFixture`'s `mountRail` sets up for U31 and U32. */
export const mountKeys = async (tree: RailNode[], neighbors: KeysNeighbors = {}) => {
  const app = createFakeApp();
  app.handlers["rail.tree"] = () => tree;

  const [now] = createSignal(NOW);
  const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, now, () => null);

  render(() => (
    <ConnectedProjectContext.Provider value={connected}>
      <Keys />
      <Rail />
      <Show when={neighbors.pane}>
        <Pane createEmulator={neighbors.pane === true ? undefined : neighbors.pane} />
      </Show>
      <Show when={neighbors.drawer}>
        <DrawerHost drawer={connected.drawer} reducedMotion={() => false} />
      </Show>
    </ConnectedProjectContext.Provider>
  ));

  await connected.rail.settled();

  return { app, rail: connected.rail, connected };
};
