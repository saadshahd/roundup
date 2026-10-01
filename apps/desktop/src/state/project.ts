import { createSignal } from "solid-js";
import type { Accessor } from "solid-js";
import type { AppSeam, DaemonExit, Project } from "../app/seam";

type Phase =
  | { kind: "loading" }
  | { kind: "empty"; failure: string | null }
  | { kind: "open"; project: Project; daemonExit: DaemonExit | null };

type ProjectState = {
  phase: Accessor<Phase>;
  /** Asks the macOS chooser for a folder and opens it as the Project; a cancel changes nothing. */
  choose(): Promise<void>;
};

export const createProjectState = (app: AppSeam): ProjectState => {
  const [phase, setPhase] = createSignal<Phase>({ kind: "loading" });

  // The adapter rejects as an RpcError; a rejection that is not an Error is a bug and is rethrown, which surfaces as an unhandled rejection rather than as text.
  const fail = (failure: Error) => setPhase({ kind: "empty", failure: failure.message });

  const load = async () => {
    try {
      const project = await app.project();

      setPhase(project ? { kind: "open", project, daemonExit: null } : { kind: "empty", failure: null });
    } catch (failure) {
      if (!(failure instanceof Error)) throw failure;

      fail(failure);
    }
  };

  void load();

  void app.onDaemonExited((daemonExit) => {
    const current = phase();

    if (current.kind === "open") setPhase({ ...current, daemonExit });
  });

  return {
    phase,
    choose: async () => {
      try {
        const path = await app.chooseProjectPath();

        if (path === null) return;

        setPhase({ kind: "open", project: await app.openProject(path), daemonExit: null });
      } catch (failure) {
        if (!(failure instanceof Error)) throw failure;

        fail(failure);
      }
    },
  };
};
