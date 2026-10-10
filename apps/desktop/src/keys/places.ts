/** The places `placeOf` distinguishes today (Help is U54's); U41's switcher arrives with U51. `"other"` is
 * anywhere keyboard focus can sit that is none of these. */
export type Place = "rail" | "pane" | "drawer" | "help" | "other";

/** Which place `target` sits in: the first of these marked ancestors found, checked in a fixed order. */
export const placeOf = (target: Element | null): Place => {
  if (target?.closest('[role="tree"]')) return "rail";

  if (target?.closest(".pane")) return "pane";

  if (target?.closest(".drawer")) return "drawer";

  if (target?.closest(".help")) return "help";

  return "other";
};
