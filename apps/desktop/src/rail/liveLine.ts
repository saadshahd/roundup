import type { Kind } from "@contracts/Kind";
import type { RailNode } from "@contracts/agent/RailNode";
import type { ExitState } from "../state/rail";
import { elapsed } from "../ink/elapsed";

const UNPROMPTED: readonly Kind[] = ["blocked", "needs-you", "error"];

/** U7's wording for a program that ended; also what U14's pane header repeats. */
const exitText = ({ code }: ExitState): string => (code === null ? "exited by signal" : `exited ${code}`);

/** The second line under a row: an Agent's label and age, an exited Terminal's ending, nothing for a running Terminal or a Group. */
export const liveLineOf = (node: RailNode, exit: ExitState | null, now: number): string | null => {
  if (node.status) return `${node.status.label}  ${elapsed(node.status.since, now)}`;

  if (node.kind === "terminal" && exit) return node.terminal_id === null ? "exited" : exitText(exit);

  return null;
};

/** Hidden until hover or selection, except when the Agent needs the user's eye. */
export const isUnprompted = (node: RailNode): boolean => node.status !== null && UNPROMPTED.includes(node.status.kind);
