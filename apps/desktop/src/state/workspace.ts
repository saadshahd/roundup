import { createContext, useContext } from "solid-js";
import type { Accessor } from "solid-js";
import { connectEvents } from "../app/events";
import type { Events } from "../app/events";
import type { AppSeam, Project } from "../app/seam";
import { createDrawer } from "../drawer/drawer";
import type { DrawerState } from "../drawer/drawer";
import { createRailState } from "./rail";
import type { RailState } from "./rail";

/** Everything a region of an open Project reads. */
export type Workspace = {
  project: Project;
  app: AppSeam;
  events: Events;
  rail: RailState;
  drawer: DrawerState;
  reducedMotion: Accessor<boolean>;
};

export const openWorkspace = async (
  app: AppSeam,
  project: Project,
  reducedMotion: Accessor<boolean>,
): Promise<Workspace> => {
  const events = await connectEvents(app);
  const rail = createRailState(app, events);

  await rail.settled();

  return { project, app, events, rail, drawer: createDrawer(), reducedMotion };
};

export const WorkspaceContext = createContext<Workspace>();

export const useWorkspace = (): Workspace => {
  const workspace = useContext(WorkspaceContext);

  if (!workspace) throw new Error("no open Project: a region read the Workspace outside one");

  return workspace;
};
