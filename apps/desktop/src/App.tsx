import { createMemo, createResource, createSignal, ErrorBoundary, For, Show } from "solid-js";
import type { Accessor } from "solid-js";
import { ChooseProject, EmptyRail, EmptyShelf } from "./app/FirstRun";
import { Layout } from "./app/Layout";
import { Reopen } from "./app/Reopen";
import type { AppSeam, DaemonExit, Project } from "./app/seam";
import { DrawerHost } from "./drawer/DrawerHost";
import { ErrorLine, failureLine } from "./ink/ErrorLine";
import { exitText } from "./ink/exitText";
import { Pads } from "./pads/Pads";
import { AttentionChip } from "./rail/AttentionChip";
import { railStorage } from "./rail/persist/storage";
import { Jump } from "./rail/Jump";
import { Rail } from "./rail/Rail";
import { Help } from "./keys/Help";
import { Keys } from "./keys/Keys";
import { createNow } from "./state/clock";
import type { Clock } from "./state/clock";
import { createProjectState } from "./state/project";
import { connectProject, ConnectedProjectContext } from "./state/connectedProject";
import { createXtermEmulators } from "./terminal/emulator";
import type { EmulatorFactory } from "./terminal/emulator";
import { Pane } from "./terminal/Pane";
import { Thread } from "./thread/Thread";
import { trackSelections } from "./thread/emulators";
import { Todos } from "./todos/Todos";

const daemonExitText = ({ code }: DaemonExit): string =>
  `daemon ${exitText(code === null ? { kind: "signal" } : { kind: "code", code })}`;

const OpenProject = (props: {
  app: AppSeam;
  project: Project;
  daemonExit: DaemonExit | null;
  reopenFailure: string | null;
  onReopen: () => void;
  reopening: boolean;
  reducedMotion: Accessor<boolean>;
  now: Accessor<number>;
  createEmulator?: EmulatorFactory | undefined;
}) => {
  const emulators = trackSelections(props.createEmulator ?? createXtermEmulators());
  const [jumpFailure, setJumpFailure] = createSignal<string | null>(null);

  const [connected] = createResource(() =>
    connectProject(props.app, props.project, props.reducedMotion, props.now, () => props.daemonExit, railStorage(props.project.path)),
  );

  return (
    <ErrorBoundary fallback={failureLine}>
      <Show when={connected()}>
        {(open) => (
          <ConnectedProjectContext.Provider value={open()}>
            <Keys />
            <Jump onFailure={setJumpFailure} />
            <Layout
              header={
                <>
                  {`roundup   ${open().project.name}`}
                  <AttentionChip />
                  <Show when={props.daemonExit}>
                    {(exit) => (
                      <>
                        {"   "}
                        <ErrorLine message={props.reopenFailure ?? daemonExitText(exit())} />
                        <Reopen onReopen={props.onReopen} busy={props.reopening} />
                      </>
                    )}
                  </Show>
                </>
              }
              rail={<Rail />}
              centre={
                <div class="centre-stack">
                  <Pane notice={open().rail.failure() ?? jumpFailure()} createEmulator={emulators.create} />
                  <Thread selectionOf={emulators.selectionOf} />
                  <Help />
                </div>
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

export const App = (props: {
  app: AppSeam;
  reducedMotion: Accessor<boolean>;
  clock: Clock;
  createEmulator?: EmulatorFactory | undefined;
}) => {
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
          <For each={[phase().generation]}>
            {() => (
              <OpenProject
                app={props.app}
                project={phase().project}
                daemonExit={phase().daemonGone?.exit ?? null}
                reopenFailure={phase().daemonGone?.reopenFailure ?? null}
                onReopen={() => void project.reopen()}
                reopening={project.reopening()}
                reducedMotion={props.reducedMotion}
                now={now}
                createEmulator={props.createEmulator}
              />
            )}
          </For>
        )}
      </Show>
    </>
  );
};
