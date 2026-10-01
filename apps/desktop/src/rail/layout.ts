import type { Kind } from "@contracts/Kind";
import type { RailNode } from "@contracts/agent/RailNode";
import { mostUrgent } from "../ink/glyph";

const DONE_FOLD_MS = 10 * 60_000;

/** What a collapsed Group shows in place of `▾`: its most urgent descendant Kind (`null` when no Agent is below) and its child count. */
export type Collapsed = { kind: Kind | null; children: number };

export type RailRow =
  | { kind: "node"; key: string; depth: number; node: RailNode; collapsed: Collapsed | null }
  | { kind: "fold"; key: string; depth: number; parent: string | null; count: number };

export type RailView = {
  collapsed: ReadonlySet<string>;
  /** Parents whose `✓ n done` line has been clicked open; `null` is the top level. */
  unfolded: ReadonlySet<string | null>;
  now: number;
  /** A selected Agent stays in its row, so the selection is never invisible. */
  selected: string | null;
};

export const isPlainGroup = (node: RailNode): boolean => node.kind === "group" && !node.meta;

const isFoldedAgent = (node: RailNode, view: RailView): boolean =>
  node.kind === "agent" &&
  node.status !== null &&
  node.status.kind === "done" &&
  view.now - node.status.since >= DONE_FOLD_MS &&
  node.id !== view.selected;

/** Rows in tree order: siblings by `order`, done Agents last under each parent, a collapsed Group without its descendants. */
export const layoutRail = (nodes: readonly RailNode[], view: RailView): RailRow[] => {
  const childrenOf = new Map<string | null, RailNode[]>();

  for (const node of nodes) childrenOf.set(node.parent, [...(childrenOf.get(node.parent) ?? []), node]);

  const under = (parent: string | null): RailNode[] =>
    (childrenOf.get(parent) ?? []).toSorted((a, b) => a.order - b.order);

  const kindsBelow = (parent: string): Kind[] =>
    under(parent).flatMap((child) => [...(child.status ? [child.status.kind] : []), ...kindsBelow(child.id)]);

  const rowsOf = (parent: string | null, depth: number): RailRow[] => {
    const siblings = under(parent);
    const folded = siblings.filter((node) => isFoldedAgent(node, view));
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
