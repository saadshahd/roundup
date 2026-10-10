import type { RailNode } from "@contracts/agent/RailNode";
import { ancestorsOf } from "../rail/layout";

/** What `←`/`→` does to the focused row: toggle its own fold, or move the focus and selection to another row. */
export type RailStep = { kind: "collapse" } | { kind: "expand" } | { kind: "select"; id: string } | { kind: "none" };

/**
 * `←`/`→` on `id` (U41). `expanded` is `true`/`false` for a row with U8's `▾`/`▸` toggle (a Workstream), `null`
 * for one with no toggle (a leaf, or a Door, which has none yet). `firstVisibleChild` is the id of the row
 * the Rail renders right after `id`, one level deeper (`undefined` when there is none): a child folded into U8's
 * `✓ n done` line is never it, because no row stands for it until the fold opens.
 */
export const railStep = (
  nodes: readonly RailNode[],
  id: string,
  direction: "left" | "right",
  expanded: boolean | null,
  firstVisibleChild: string | undefined,
): RailStep => {
  if (direction === "left") {
    if (expanded === true) return { kind: "collapse" };

    const parent = ancestorsOf(nodes, id)[0];

    return parent === undefined ? { kind: "none" } : { kind: "select", id: parent };
  }

  if (expanded === false) return { kind: "expand" };

  return firstVisibleChild === undefined ? { kind: "none" } : { kind: "select", id: firstVisibleChild };
};
