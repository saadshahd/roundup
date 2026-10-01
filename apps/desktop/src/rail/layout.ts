import type { Kind } from "@contracts/Kind";
import type { RailNode } from "@contracts/agent/RailNode";
import { mostUrgent } from "../ink/glyph";

const DONE_FOLD_MS = 10 * 60_000;

/** What a collapsed Group shows in place of `▾`: its most urgent descendant Kind (`null` when no Agent is below) and its child count. */
export type Collapsed = { kind: Kind | null; children: number };

export type RailRow =
  | {
      kind: "node";
      key: string;
      depth: number;
      node: RailNode;
      collapsed: Collapsed | null;
      /** An Agent shown only because its `✓ n done` line is open: it sits after its live siblings whatever its `order`. */
      folded: boolean;
    }
  | { kind: "fold"; key: string; depth: number; parent: string | null; count: number };

export type NodeRow = Extract<RailRow, { kind: "node" }>;

export type RailView = {
  collapsed: ReadonlySet<string>;
  /** Parents whose `✓ n done` line has been clicked open; `null` is the top level. */
  unfolded: ReadonlySet<string | null>;
  now: number;
};

export const isPlainGroup = (node: RailNode): boolean => node.kind === "group" && !node.meta;

/** The children of `parent` (`null` is the top level) by `order`. */
export const siblingsOf = (nodes: readonly RailNode[], parent: string | null): RailNode[] =>
  nodes.filter((node) => node.parent === parent).toSorted((a, b) => a.order - b.order);

/** Every node below `id`, each parent before its children. */
export const descendantsOf = (nodes: readonly RailNode[], id: string): RailNode[] =>
  siblingsOf(nodes, id).flatMap((child) => [child, ...descendantsOf(nodes, child.id)]);

const isFoldedAgent = (node: RailNode, now: number): boolean =>
  node.kind === "agent" &&
  node.status !== null &&
  node.status.kind === "done" &&
  now - node.status.since >= DONE_FOLD_MS;

/** Rows in tree order: siblings by `order`, then Agents done for 10 minutes or more as one `✓ n done` line, a collapsed Group without its descendants. */
export const layoutRail = (nodes: readonly RailNode[], view: RailView): RailRow[] => {
  const under = (parent: string | null): RailNode[] => siblingsOf(nodes, parent);

  const kindsBelow = (parent: string): Kind[] =>
    descendantsOf(nodes, parent).flatMap((child) => (child.status ? [child.status.kind] : []));

  const rowsOf = (parent: string | null, depth: number): RailRow[] => {
    const siblings = under(parent);
    const folded = siblings.filter((node) => isFoldedAgent(node, view.now));
    const unfolded = view.unfolded.has(parent);

    const shown = [
      ...siblings.filter((node) => !folded.includes(node)),
      ...(unfolded ? folded : []),
    ];

    const nodeRows = shown.flatMap((node): RailRow[] => {
      const collapsed = isPlainGroup(node) && view.collapsed.has(node.id);

      const row: RailRow = {
        kind: "node",
        key: `node:${node.id}`,
        depth,
        node,
        folded: folded.includes(node),
        collapsed: collapsed ? { kind: mostUrgent(kindsBelow(node.id)), children: under(node.id).length } : null,
      };

      return collapsed ? [row] : [row, ...rowsOf(node.id, depth + 1)];
    });

    const foldRow: RailRow[] =
      folded.length > 0 && !unfolded
        ? [{ kind: "fold", key: `fold:${parent ?? ""}`, depth, parent, count: folded.length }]
        : [];

    return [...nodeRows, ...foldRow];
  };

  return rowsOf(null, 0);
};
