import { createSignal } from "solid-js";
import type { Accessor } from "solid-js";
import type { AppSeam, DaemonExit, Project } from "../app/seam";

export type Phase =
  | { kind: "loading" }
  | { kind: "empty"; failure: string | null }
  | { kind: "open"; project: Project; daemonExit: DaemonExit | null };

export type ProjectState = {
  phase: Accessor<Phase>;
  /** Asks the macOS chooser for a folder and opens it as the Project; a cancel changes nothing. */
  choose(): Promise<void>;
};

export const createProjectState = (app: AppSeam): ProjectState => {
  const [phase, setPhase] = createSignal<Phase>({ kind: "loading" });

  void app.project().then((project) => {
    setPhase(project ? { kind: "open", project, daemonExit: null } : { kind: "empty", failure: null });
  });

  void app.onDaemonExited((daemonExit) => {
    const current = phase();

    if (current.kind === "open") setPhase({ ...current, daemonExit });
  });

  return {
    phase,
    choose: async () => {
      const path = await app.chooseProjectPath();

      if (path === null) return;

      try {
        setPhase({ kind: "open", project: await app.openProject(path), daemonExit: null });
      } catch (failure) {
        if (!(failure instanceof Error)) throw failure;

        setPhase({ kind: "empty", failure: failure.message });
      }
    },
  };
};
