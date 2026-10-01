import type { RailNode } from "@contracts/agent/RailNode";
import { elapsed } from "../ink/elapsed";
import { exitText } from "../ink/exitText";
import type { ExitState } from "../state/rail";

/** The pane's top line for the selected row; `null` for a plain Group, which has no Terminal. */
export const paneHeader = (node: RailNode, exit: ExitState | null, now: number): string | null => {
  if (node.status !== null) return `${node.name}  ${node.status.kind} ${elapsed(node.status.since, now)}`;

  if (node.kind !== "terminal") return null;

  return exit === null ? node.name : `${node.name}  ${exitText(exit)}`;
};

/** A row whose program runs: an Agent, a Meta-agent or a Terminal that has not exited. */
export const isStoppable = (node: RailNode, exit: ExitState | null): boolean =>
  exit === null && (node.status !== null || node.kind === "terminal");
