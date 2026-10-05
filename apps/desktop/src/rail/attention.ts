import type { RailNode } from "@contracts/agent/RailNode";

/** Agents and Doors that need the user, most urgent first: Kind `error` before `needs-you`, then the oldest Status. Terminals have no Kind and never count. */
const attentionOrder = (nodes: readonly RailNode[]): RailNode[] =>
  nodes
    .flatMap((node) => {
      const kind = node.status?.kind;

      return node.status && (kind === "error" || kind === "needs-you")
        ? [{ node, rank: kind === "error" ? 0 : 1, since: node.status.since }]
        : [];
    })
    .sort((a, b) => a.rank - b.rank || a.since - b.since)
    .map(({ node }) => node);

export const attentionCount = (nodes: readonly RailNode[]): number => attentionOrder(nodes).length;

/** The first of `attentionOrder` when `selected` is not in it; otherwise the next, wrapping after the last. `null` when none need you. */
export const nextAttention = (nodes: readonly RailNode[], selected: string | null): RailNode | null => {
  const order = attentionOrder(nodes);
  const at = order.findIndex((node) => node.id === selected);

  return order[(at + 1) % order.length] ?? null;
};
