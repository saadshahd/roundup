import { render, screen } from "@solidjs/testing-library";
import { createSignal, Show } from "solid-js";
import type { Accessor } from "solid-js";
import type { DaemonExit } from "../app/seam";
import type { RailNode } from "@contracts/agent/RailNode";
import type { TerminalInfo } from "@contracts/terminal/TerminalInfo";
import { createFakeApp } from "../testing/fakeApp";
import { info, NOW } from "../testing/nodes";
import { ConnectedProjectContext, connectProject } from "../state/connectedProject";
import { DrawerHost } from "../drawer/DrawerHost";
import { Pane } from "../terminal/Pane";
import type { EmulatorFactory } from "../terminal/emulator";
import { AttentionChip } from "./AttentionChip";
import { liveTitleOf } from "./liveLine";
import { Rail } from "./Rail";

export const exitedTerminal = (id: string, exit_code: number | null): TerminalInfo =>
  info(`t-${id}`, { running: false, exit_code });

/** What else to mount alongside the Rail: a Pane (an `EmulatorFactory` gives it a focusable screen) or the DrawerHost. */
type RailNeighbors = { pane?: true | EmulatorFactory; drawer?: boolean };

/** The Rail on a fake Daemon whose tree is `tree` and whose clock is a signal the test can advance. */
export const mountRail = async (
  tree: RailNode[],
  terminals: TerminalInfo[] = [],
  daemonExit: Accessor<DaemonExit | null> = () => null,
  reducedMotion: () => boolean = () => false,
  neighbors: RailNeighbors = {},
) => {
  const app = createFakeApp();
  app.handlers["rail.tree"] = () => tree;
  app.handlers["terminal.list"] = () => terminals;

  const [now, setNow] = createSignal(NOW);
  const connected = await connectProject(app, { name: "p", path: "/p" }, reducedMotion, now, daemonExit);

  render(() => (
    <ConnectedProjectContext.Provider value={connected}>
      <AttentionChip />
      <Rail />
      <Show when={neighbors.pane}>
        <Pane createEmulator={neighbors.pane === true ? undefined : neighbors.pane} />
      </Show>
      <Show when={neighbors.drawer}>
        <DrawerHost drawer={connected.drawer} reducedMotion={reducedMotion} />
      </Show>
    </ConnectedProjectContext.Provider>
  ));

  return { app, rail: connected.rail, connected, setNow };
};

export const rowOf = (name: string): HTMLElement => {
  const row = screen.getByText(name).closest<HTMLElement>("[role=treeitem]");

  if (!row) throw new Error(`no row for ${name}`);

  return row;
};

export const shownLiveLine = (name: string): string | null => {
  const live = rowOf(name).querySelector(".live");
  const label = live?.querySelector(".live-label")?.textContent;
  const age = live?.querySelector(".live-age")?.textContent;

  return label != null && age != null ? liveTitleOf({ label, age }) : (live?.textContent ?? null);
};

export const glyphOf = (name: string): HTMLElement => {
  const glyph = rowOf(name).querySelector<HTMLElement>(".glyph");

  if (!glyph) throw new Error(`no glyph on ${name}`);

  return glyph;
};

export const rowNames = (): string[] =>
  screen.getAllByRole("treeitem").map((row) => row.querySelector(".name")?.textContent ?? "");

export const tabbableRows = (): HTMLElement[] => screen.getAllByRole("treeitem").filter((row) => row.tabIndex === 0);

export const railCallsTo = (app: ReturnType<typeof createFakeApp>, method: string) =>
  app.calls.filter((call) => call.method === method).map((call) => call.params);
