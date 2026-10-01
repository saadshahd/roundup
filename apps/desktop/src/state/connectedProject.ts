import { createContext, useContext } from "solid-js";
import type { Accessor } from "solid-js";
import { connectEvents } from "../app/events";
import type { Events } from "../app/events";
import type { AppSeam, Project } from "../app/seam";
import { createDrawer } from "../drawer/drawer";
import type { DrawerState } from "../drawer/drawer";
import { createRailState } from "./rail";
import type { RailState } from "./rail";

/** Everything a region of an open Project reads: the Project and its live connection to the Daemon. */
export type ConnectedProject = {
  project: Project;
  app: AppSeam;
  events: Events;
  rail: RailState;
  drawer: DrawerState;
  reducedMotion: Accessor<boolean>;
  /** Milliseconds since the epoch, refreshed on a coarse tick; for elapsed times. */
  now: Accessor<number>;
};

export const connectProject = async (
  app: AppSeam,
  project: Project,
  reducedMotion: Accessor<boolean>,
  now: Accessor<number>,
): Promise<ConnectedProject> => {
  const events = await connectEvents(app);
  const rail = createRailState(app, events);

  await rail.settled();

  return { project, app, events, rail, drawer: createDrawer(), reducedMotion, now };
};

export const ConnectedProjectContext = createContext<ConnectedProject>();

export const useConnectedProject = (): ConnectedProject => {
  const connected = useContext(ConnectedProjectContext);

  if (!connected) throw new Error("no open Project: a region read the ConnectedProject outside one");

  return connected;
};
