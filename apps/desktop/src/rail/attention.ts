import type { RailNode } from "@contracts/agent/RailNode";

/** Agents and Meta-agents that need the user: Kind `needs-you` or `error`. Terminals have no Kind and never count. */
export const attentionCount = (nodes: readonly RailNode[]): number =>
  nodes.filter((node) => node.status?.kind === "needs-you" || node.status?.kind === "error").length;
