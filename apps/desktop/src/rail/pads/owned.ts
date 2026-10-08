import type { Actor } from "@contracts/Actor";
import type { RailNode } from "@contracts/agent/RailNode";
import type { Pad } from "@contracts/pad/Pad";

/** True when an Agent that owns `pad` is a row on the Rail (U58). A Pad of the user, an Extension, or a removed Agent is not. */
export const isOwnedOnRail = (pad: Pad, nodes: readonly RailNode[]): boolean =>
  pad.owner.kind === "agent" && nodes.some((node) => node.kind === "agent" && node.id === pad.owner.id);

/** The Pads `agent` owns, in `pad.list` order. */
export const padsOwnedBy = (pads: readonly Pad[], agent: Actor["id"]): Pad[] =>
  pads.filter((pad) => pad.owner.kind === "agent" && pad.owner.id === agent);
