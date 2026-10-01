import { render, screen } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import type { Actor } from "@contracts/Actor";
import type { Event as DaemonEvent } from "@contracts/Event";
import type { EventData } from "@contracts/EventData";
import type { Kind } from "@contracts/Kind";
import type { RailNode } from "@contracts/agent/RailNode";
import type { TerminalInfo } from "@contracts/terminal/TerminalInfo";
import { createFakeApp } from "../testing/fakeApp";
import { openWorkspace, WorkspaceContext } from "../state/workspace";
import { Rail } from "./Rail";

export const NOW = 1_700_000_000_000;

export const MINUTE = 60_000;

const USER: Actor = { kind: "user", id: "you", parent: null };

export const event = (data: EventData): DaemonEvent => ({ actor: USER, ...data });

export const agent = (id: string, kind: Kind, label: string, over: Partial<RailNode> = {}): RailNode => ({
  id,
  kind: "agent",
  name: id,
  parent: null,
  order: 0,
  status: { kind, label, since: NOW },
  meta: false,
  terminal_id: `t-${id}`,
  ...over,
});

export const group = (id: string, over: Partial<RailNode> = {}): RailNode => ({
  id,
  kind: "group",
  name: id,
  parent: null,
  order: 0,
  status: null,
  meta: false,
  terminal_id: null,
  ...over,
});

export const terminal = (id: string, over: Partial<RailNode> = {}): RailNode => ({
  id,
  kind: "terminal",
  name: id,
  parent: null,
  order: 0,
  status: null,
  meta: false,
  terminal_id: `t-${id}`,
  ...over,
});

export const exitedTerminal = (id: string, exit_code: number | null): TerminalInfo => ({
  id: `t-${id}`,
  cwd: "/p",
  title: null,
  running: false,
  exit_code,
});

/** The Rail on a fake Daemon whose tree is `tree` and whose clock is a signal the test can advance. */
export const mountRail = async (tree: RailNode[], terminals: TerminalInfo[] = []) => {
  const app = createFakeApp();
  app.handlers["rail.tree"] = () => tree;
  app.handlers["terminal.list"] = () => terminals;

  const workspace = await openWorkspace(app, { name: "p", path: "/p" }, () => false);
  const [now, setNow] = createSignal(NOW);

  render(() => (
    <WorkspaceContext.Provider value={workspace}>
      <Rail now={now} />
    </WorkspaceContext.Provider>
  ));

  return { app, rail: workspace.rail, setNow };
};

export const rowOf = (name: string): HTMLElement => {
  const row = screen.getByText(name).closest<HTMLElement>("[role=treeitem]");

  if (!row) throw new Error(`no row for ${name}`);

  return row;
};

export const liveLineOf = (name: string): string | null => rowOf(name).querySelector(".live")?.textContent ?? null;

export const glyphOf = (name: string): HTMLElement => {
  const glyph = rowOf(name).querySelector<HTMLElement>(".glyph");

  if (!glyph) throw new Error(`no glyph on ${name}`);

  return glyph;
};

export const rowNames = (): string[] =>
  screen.getAllByRole("treeitem").map((row) => row.querySelector(".name")?.textContent ?? "");

export const callsTo = (app: ReturnType<typeof createFakeApp>, method: string) =>
  app.calls.filter((call) => call.method === method).map((call) => call.params);
