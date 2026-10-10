import type { RailNode } from "@contracts/agent/RailNode";

const STEM = "workstream";

/** U167: the first free name of `workstream`, `workstream 2`, `workstream 3`… among the Rail's root rows. */
export const defaultWorkstreamName = (nodes: readonly RailNode[]): string => {
  const taken = new Set(nodes.flatMap((node) => (node.parent === null ? [node.name] : [])));

  for (let n = 1; ; n += 1) {
    const name = n === 1 ? STEM : `${STEM} ${n}`;

    if (!taken.has(name)) return name;
  }
};
