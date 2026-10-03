import type { Kind } from "@contracts/Kind";
import type { RailNode } from "@contracts/agent/RailNode";
import type { ExitState } from "../state/rail";
import { elapsed } from "../ink/elapsed";
import { exitText } from "../ink/exitText";

const UNPROMPTED: readonly Kind[] = ["blocked", "needs-you", "error"];

/** The Live line's label and, for an Agent or Meta-agent, its elapsed time pinned after it (U7); an exited Terminal's ending has no age. */
export type LiveLine = { label: string; age: string | null };

/** What the row shows on its Live line, or `null` when it shows none. */
export const liveLineOf = (node: RailNode, exit: ExitState | null, now: number): LiveLine | null => {
  if (node.status) return { label: node.status.label, age: elapsed(node.status.since, now) };

  if (node.kind === "terminal" && exit) return { label: exitText(exit), age: null };

  return null;
};

/** The Live line's `title`: the whole label and age, which the row may cut. */
export const liveTitleOf = (line: LiveLine): string => (line.age !== null ? `${line.label}  ${line.age}` : line.label);

/** Hidden until hover or selection, except when the Agent needs the user's eye. */
export const isUnprompted = (node: RailNode): boolean => node.status !== null && UNPROMPTED.includes(node.status.kind);
