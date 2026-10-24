import { captureFocus } from "../keys/focus";
import type { ActivationSource } from "../keys/registry";
import { batch, createSignal } from "solid-js";
import type { Accessor, JSX } from "solid-js";

export type DrawerContent = () => JSX.Element;

export type DrawerState = {
  /** The one Drawer that is open, or `null`. */
  content: Accessor<DrawerContent | null>;
  /** Replaces whatever Drawer is open. */
  open(content: DrawerContent, source?: ActivationSource): void;
  close(source?: ActivationSource): void;
  source: Accessor<ActivationSource>;
};

export const DRAWER_EASING = "var(--easing-out)";

export const createDrawer = (): DrawerState => {
  const [content, setContent] = createSignal<DrawerContent | null>(null);

  const [source, setSource] = createSignal<ActivationSource>("pointer");

  return {
    content,
    source,
    open: (next, from = "pointer") => batch(() => { captureFocus(true); setSource(from); setContent(() => next); }),
    close: (from = "pointer") => batch(() => { setSource(from); setContent(null); }),
  };
};
