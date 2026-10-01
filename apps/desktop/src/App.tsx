import { createMemo, createResource, ErrorBoundary, Show } from "solid-js";
import type { Accessor } from "solid-js";
import { ChooseProject, EmptyRail, EmptyShelf } from "./app/FirstRun";
import { Layout } from "./app/Layout";
import type { AppSeam, DaemonExit, Project } from "./app/seam";
import { DrawerHost } from "./drawer/DrawerHost";
import { ErrorLine } from "./ink/ErrorLine";
import { Pads } from "./pads/Pads";
import { Rail } from "./rail/Rail";
import { createProjectState } from "./state/project";
import { openWorkspace, WorkspaceContext } from "./state/workspace";
import { Pane } from "./terminal/Pane";
import { Todos } from "./todos/Todos";

const daemonExitText = ({ code }: DaemonExit): string =>
  code === null ? "daemon exited" : `daemon exited ${code}`;

const OpenProject = (props: {
  app: AppSeam;
  project: Project;
  daemonExit: DaemonExit | null;
  reducedMotion: Accessor<boolean>;
}) => {
  const [workspace] = createResource(() => openWorkspace(props.app, props.project, props.reducedMotion));

  return (
    <ErrorBoundary fallback={(failure) => <ErrorLine message={failure instanceof Error ? failure.message : String(failure)} />}>
      <Show when={workspace()}>
        {(open) => (
          <WorkspaceContext.Provider value={open()}>
            <Layout
              header={`roundup   ${open().project.name}`}
              rail={<Rail />}
              centre={
                <>
                  <Show when={props.daemonExit}>{(exit) => <ErrorLine message={daemonExitText(exit())} />}</Show>
                  <Pane />
                </>
              }
              shelf={
                <>
                  <Todos />
                  <Pads />
                </>
              }
              overlay={<DrawerHost drawer={open().drawer} reducedMotion={open().reducedMotion} />}
            />
          </WorkspaceContext.Provider>
        )}
      </Show>
    </ErrorBoundary>
  );
};

export const App = (props: { app: AppSeam; reducedMotion: Accessor<boolean> }) => {
  const project = createProjectState(props.app);

  const empty = createMemo(() => {
    const phase = project.phase();

    return phase.kind === "empty" ? phase : undefined;
  });

  const open = createMemo(() => {
    const phase = project.phase();

    return phase.kind === "open" ? phase : undefined;
  });

  return (
    <>
      <Show when={empty()}>
        {(phase) => (
          <Layout
            header="roundup"
            rail={<EmptyRail />}
            centre={<ChooseProject failure={phase().failure} onChoose={() => void project.choose()} />}
            shelf={<EmptyShelf />}
            overlay={null}
          />
        )}
      </Show>
      <Show when={open()}>
        {(phase) => (
          <OpenProject
            app={props.app}
            project={phase().project}
            daemonExit={phase().daemonExit}
            reducedMotion={props.reducedMotion}
          />
        )}
      </Show>
    </>
  );
};
