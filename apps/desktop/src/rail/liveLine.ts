import type { Kind } from "@contracts/Kind";
import type { RailNode } from "@contracts/agent/RailNode";
import type { ExitState } from "../state/rail";
import { elapsed } from "../ink/elapsed";
import { exitText } from "../ink/exitText";

const UNPROMPTED: readonly Kind[] = ["blocked", "needs-you", "error"];

/** The second line under a row: an Agent's label and age, an exited Terminal's ending, nothing for a running Terminal or a plain Group. */
export const liveLineOf = (node: RailNode, exit: ExitState | null, now: number): string | null => {
  if (node.status) return `${node.status.label}  ${elapsed(node.status.since, now)}`;

  if (node.kind === "terminal" && exit) return exitText(exit);

  return null;
};

/** Hidden until hover or selection, except when the Agent needs the user's eye. */
export const isUnprompted = (node: RailNode): boolean => node.status !== null && UNPROMPTED.includes(node.status.kind);
