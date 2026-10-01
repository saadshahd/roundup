import type { Actor } from "@contracts/Actor";

export const USER: Actor = { kind: "user", id: "you", parent: null };

/** `◇` for the user's Pad, `◈` for any other owner. These are not Glyphs: Glyphs mark Kinds. */
export const ownerMark = (owner: Actor): string => (owner.kind === "user" ? "◇" : "◈");
