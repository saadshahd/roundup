import { createSignal } from "solid-js";
import type { Accessor } from "solid-js";
import type { AppSeam, DaemonExit, Project } from "../app/seam";

type Phase =
  | { kind: "loading" }
  | { kind: "empty"; failure: string | null }
  | {
      kind: "open";
      project: Project;
      /** Set once the Daemon has ended; `reopenFailure` is why the last reopen failed and replaces the exit text until one succeeds. */
      daemonGone: { exit: DaemonExit; reopenFailure: string | null } | null;
      /** Counts reopens, so a new Daemon's connection replaces the old one whole. */
      generation: number;
    };

type ProjectState = {
  phase: Accessor<Phase>;
  /** Asks the macOS chooser for a folder and opens it as the Project; a cancel changes nothing. */
  choose(): Promise<void>;
  /** Starts a new Daemon for the open Project (U37); a failure keeps the exit and records why. */
  reopen(): Promise<void>;
};

export const createProjectState = (app: AppSeam): ProjectState => {
  const [phase, setPhase] = createSignal<Phase>({ kind: "loading" });

  // The adapter rejects as an RpcError; a rejection that is not an Error is a bug and is rethrown, which surfaces as an unhandled rejection rather than as text.
  const fail = (failure: Error) => setPhase({ kind: "empty", failure: failure.message });

  const load = async () => {
    try {
      const project = await app.project();

      setPhase(project ? { kind: "open", project, daemonGone: null, generation: 0 } : { kind: "empty", failure: null });
    } catch (failure) {
      if (!(failure instanceof Error)) throw failure;

      fail(failure);
    }
  };

  void load();

  void app.onDaemonExited((exit) => {
    const current = phase();

    if (current.kind === "open") setPhase({ ...current, daemonGone: { exit, reopenFailure: null } });
  });

  return {
    phase,
    choose: async () => {
      try {
        const path = await app.chooseProjectPath();

        if (path === null) return;

        setPhase({ kind: "open", project: await app.openProject(path), daemonGone: null, generation: 0 });
      } catch (failure) {
        if (!(failure instanceof Error)) throw failure;

        fail(failure);
      }
    },
    reopen: async () => {
      const current = phase();

      if (current.kind !== "open") return;

      try {
        const project = await app.openProject(current.project.path);

        setPhase({ kind: "open", project, daemonGone: null, generation: current.generation + 1 });
      } catch (failure) {
        if (!(failure instanceof Error)) throw failure;

        const latest = phase();

        if (latest.kind === "open" && latest.daemonGone) setPhase({ ...latest, daemonGone: { ...latest.daemonGone, reopenFailure: failure.message } });
      }
    },
  };
};
