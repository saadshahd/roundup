import type { RailNode } from "@contracts/agent/RailNode";
import { descendantsOf, isWorkstream, siblingsOf } from "../rail/layout";

export type Home = { home: string | null; name: string };

const PROJECT_ROOT = "project root";

/** U158: a Workstream's current name from the Rail; the Project root for `null`, a Rail not yet loaded or an id the Rail does not hold — never the id. */
export const homeNameOf = (nodes: readonly RailNode[], home: string | null): string =>
  nodes.find((node) => node.id === home && isWorkstream(node))?.name ?? PROJECT_ROOT;

/** U157: the Project root, then each Workstream in Rail order, the current Home left out. */
export const homesToOffer = (nodes: readonly RailNode[], current: string | null): Home[] => {
  const workstreams = siblingsOf(nodes, null).flatMap((top) => [top, ...descendantsOf(nodes, top.id)]).filter(isWorkstream);
  const offers: Home[] = [{ home: null, name: PROJECT_ROOT }, ...workstreams.map((workstream) => ({ home: workstream.id, name: workstream.name }))];
  const present = homeNameOf(nodes, current) === PROJECT_ROOT ? null : current;

  return offers.filter((offer) => offer.home !== present);
};
