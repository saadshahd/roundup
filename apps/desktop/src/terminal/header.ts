import type { RailNode } from "@contracts/agent/RailNode";
import { elapsed } from "../ink/elapsed";
import type { ExitState } from "../state/rail";

const exitWording = (exit: ExitState): string =>
  exit.kind === "code" ? `exited ${exit.code}` : exit.kind === "signal" ? "exited by signal" : "exited";

/** The pane's top line for the selected row; `null` for a plain Group, which has no Terminal. */
export const paneHeader = (node: RailNode, exit: ExitState | null, now: number): string | null => {
  if (node.status !== null) return `${node.name}  ${node.status.kind} ${elapsed(node.status.since, now)}`;

  if (node.kind !== "terminal") return null;

  return exit === null ? node.name : `${node.name}  ${exitWording(exit)}`;
};
