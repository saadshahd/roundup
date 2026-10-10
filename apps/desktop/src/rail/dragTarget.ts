import type { RailNode } from "@contracts/agent/RailNode";
import { descendantsOf, siblingsOf } from "./layout";
import type { NodeRow } from "./layout";

/** The `rail.move` arguments for a release, plus the depth the drop line is drawn at. */
export type Drop = { parent: string | null; index: number; depth: number };

/** The dragged node and the nodes that move with it. */
export const movingWith = (nodes: readonly RailNode[], dragged: string): Set<string> =>
  new Set([dragged, ...descendantsOf(nodes, dragged).map((node) => node.id)]);

/** The rows a dragged row can land between: not itself or what moves with it, since a Workstream cannot move into its own descendant. */
export const remainingRows = (nodes: readonly RailNode[], rows: readonly NodeRow[], dragged: string): NodeRow[] => {
  const moving = movingWith(nodes, dragged);

  return rows.filter((row) => !moving.has(row.node.id));
};

/** Where the dragged row lands in gap `slot` (0 is above the first row) of `rows`, with the pointer at `depth`, clamped to the depths the gap allows. `index` counts siblings without the dragged row. */
export const dropAt = (
  nodes: readonly RailNode[],
  rows: readonly NodeRow[],
  dragged: string,
  slot: number,
  depth: number,
): Drop => {
  const above = rows[slot - 1];
  const below = rows[slot];

  if (!above) return { parent: null, index: 0, depth: 0 };

  const firstChild: Drop = { parent: above.node.id, index: 0, depth: above.depth + 1 };

  if (below && below.depth > above.depth) return firstChild;

  const holds = above.node.kind === "workstream" && above.collapsed === null;
  const wanted = Math.min(Math.max(depth, below?.depth ?? 0), above.depth + (holds ? 1 : 0));

  if (wanted > above.depth) return firstChild;

  const anchor = rows.slice(0, slot).findLast((row) => row.depth === wanted);

  if (!anchor) throw new Error(`no row at depth ${wanted} above gap ${slot}`);

  const siblings = siblingsOf(nodes, anchor.node.parent).filter((node) => node.id !== dragged);

  return { parent: anchor.node.parent, index: siblings.findIndex((node) => node.id === anchor.node.id) + 1, depth: wanted };
};

/** Releasing the row at the position it already has changes nothing, so no call is worth making. */
export const isInPlace = (nodes: readonly RailNode[], dragged: string, drop: Drop): boolean => {
  const node = nodes.find((each) => each.id === dragged);

  if (!node) throw new Error(`no node ${dragged}`);

  return node.parent === drop.parent && siblingsOf(nodes, node.parent).findIndex((each) => each.id === dragged) === drop.index;
};
