/** The four places U41 lists; `"other"` is anywhere keyboard focus can sit that is none of them. */
export type Place = "rail" | "pane" | "drawer" | "other";

/** Which place `target` sits in, by its nearest marked ancestor. */
export const placeOf = (target: Element | null): Place => {
  if (target?.closest('[role="tree"]')) return "rail";

  if (target?.closest(".pane")) return "pane";

  if (target?.closest(".drawer")) return "drawer";

  return "other";
};
