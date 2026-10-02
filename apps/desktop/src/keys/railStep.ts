import type { RailNode } from "@contracts/agent/RailNode";
import { ancestorsOf, siblingsOf } from "../rail/layout";

/** What `←`/`→` does to the focused row: toggle its own fold, or move the focused row entirely. */
export type RailStep = { kind: "collapse" } | { kind: "expand" } | { kind: "select"; id: string } | { kind: "none" };

/**
 * `←`/`→` on `id` (U41). `expanded` is `true`/`false` for a row with U8's `▾`/`▸` toggle (a plain Group), `null`
 * for one with no toggle (a leaf, or a Meta-agent, which has none yet).
 */
export const railStep = (
  nodes: readonly RailNode[],
  id: string,
  direction: "left" | "right",
  expanded: boolean | null,
): RailStep => {
  if (direction === "left") {
    if (expanded === true) return { kind: "collapse" };

    const parent = ancestorsOf(nodes, id)[0];

    return parent === undefined ? { kind: "none" } : { kind: "select", id: parent };
  }

  if (expanded === false) return { kind: "expand" };

  const child = siblingsOf(nodes, id)[0];

  return child === undefined ? { kind: "none" } : { kind: "select", id: child.id };
};
