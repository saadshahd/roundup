import type { Actor } from "@contracts/Actor";
import type { Event as DaemonEvent } from "@contracts/Event";
import type { EventData } from "@contracts/EventData";
import type { Kind } from "@contracts/Kind";
import type { Order } from "@contracts/agent/Order";
import type { RailNode } from "@contracts/agent/RailNode";
import type { TerminalInfo } from "@contracts/terminal/TerminalInfo";

export const NOW = 1_700_000_000_000;

export const MINUTE = 60_000;

export const USER: Actor = { kind: "user", id: "you", parent: null };

export const event = (data: EventData): DaemonEvent => ({ actor: USER, ...data });

/** O1: the Clarification order of a node that was given none. */
export const clarifying = (question: string): Order => ({ kind: "clarification", question });

/** An Agent that started working at time 0; the other builders refine it. */
export const node = (id: string, over: Partial<RailNode> = {}): RailNode => ({
  id,
  kind: "agent",
  name: id,
  parent: null,
  order: 0,
  status: { kind: "working", label: "starting", since: 0 },
  attempt: "1",
  status_revision: "1",
  terminal_id: `t-${id}`,
  worktree: null,
  can_resume: false,
  channel: null,
  work: over.kind === "terminal" ? null : clarifying(over.kind === "room" ? "What is this Room for?" : "What should this Agent do?"),
  ...over,
});

export const agent = (id: string, kind: Kind, label: string, over: Partial<RailNode> = {}): RailNode =>
  node(id, { status: { kind, label, since: NOW }, ...over });

export const room = (id: string, over: Partial<RailNode> = {}): RailNode =>
  node(id, { kind: "room", status: { kind: "done", label: "terminal gone", since: NOW }, attempt: null, status_revision: null, terminal_id: null, ...over });

/** A Door as the Daemon sends it: a Room with a live Agent sitting at it. */
export const door = (id: string, kind: Kind, label: string, over: Partial<RailNode> = {}): RailNode =>
  room(id, { attempt: "1", status_revision: "1", status: { kind, label, since: NOW }, terminal_id: `t-${id}`, ...over });

export const terminal = (id: string, over: Partial<RailNode> = {}): RailNode =>
  node(id, { kind: "terminal", status: null, attempt: null, status_revision: null, ...over });

export const info = (id: string, over: Partial<TerminalInfo> = {}): TerminalInfo => ({
  id,
  cwd: "/p",
  title: null,
  running: true,
  exit_code: null,
  ...over,
});
