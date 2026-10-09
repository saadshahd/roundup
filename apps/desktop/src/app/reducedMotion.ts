import { createSignal, onCleanup } from "solid-js";
import type { Accessor } from "solid-js";

type Change = { matches: boolean };

/** The part of `MediaQueryList` this module uses, so a test can stand in for it. */
export type MediaQuery = {
  matches: boolean;
  addEventListener(type: "change", listener: (change: Change) => void): void;
  removeEventListener(type: "change", listener: (change: Change) => void): void;
};

/** Follows the webview's `prefers-reduced-motion` setting; `matchMedia` is injected so tests can set it. */
export const createReducedMotion = (matchMedia: (query: string) => MediaQuery): Accessor<boolean> => {
  const query = matchMedia("(prefers-reduced-motion: reduce)");
  const [reduced, setReduced] = createSignal(query.matches);
  const follow = (change: Change) => setReduced(change.matches);

  query.addEventListener("change", follow);
  onCleanup(() => query.removeEventListener("change", follow));

  return reduced;
};
