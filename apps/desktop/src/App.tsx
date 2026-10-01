import { createMemo, createResource, ErrorBoundary, Show } from "solid-js";
import type { Accessor } from "solid-js";
import { ChooseProject, EmptyRail, EmptyShelf } from "./app/FirstRun";
import { Layout } from "./app/Layout";
import type { AppSeam, DaemonExit, Project } from "./app/seam";
import { DrawerHost } from "./drawer/DrawerHost";
import { ErrorLine, failureLine } from "./ink/ErrorLine";
import { exitText } from "./ink/exitText";
import { Pads } from "./pads/Pads";
import { Rail } from "./rail/Rail";
import { createNow } from "./state/clock";
import type { Clock } from "./state/clock";
import { createProjectState } from "./state/project";
import { connectProject, ConnectedProjectContext } from "./state/connectedProject";
import { Pane } from "./terminal/Pane";
import { Todos } from "./todos/Todos";

const daemonExitText = ({ code }: DaemonExit): string =>
  `daemon ${exitText(code === null ? { kind: "signal" } : { kind: "code", code })}`;

const OpenProject = (props: {
  app: AppSeam;
  project: Project;
  daemonExit: DaemonExit | null;
  reducedMotion: Accessor<boolean>;
  now: Accessor<number>;
}) => {
  const [connected] = createResource(() =>
    connectProject(props.app, props.project, props.reducedMotion, props.now),
  );

  return (
    <ErrorBoundary fallback={failureLine}>
      <Show when={connected()}>
        {(open) => (
          <ConnectedProjectContext.Provider value={open()}>
            <Layout
              header={`roundup   ${open().project.name}`}
              rail={<Rail />}
              centre={
                <>
                  <Show when={open().rail.failure()}>{(message) => <ErrorLine message={message()} />}</Show>
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
          </ConnectedProjectContext.Provider>
        )}
      </Show>
    </ErrorBoundary>
  );
};

export const App = (props: { app: AppSeam; reducedMotion: Accessor<boolean>; clock: Clock }) => {
  const project = createProjectState(props.app);
  const now = createNow(props.clock);

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
            now={now}
          />
        )}
      </Show>
    </>
  );
};
