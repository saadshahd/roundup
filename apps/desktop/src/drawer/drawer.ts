import { createSignal } from "solid-js";
import type { Accessor, JSX } from "solid-js";

export type DrawerContent = () => JSX.Element;

export type DrawerState = {
  /** The one Drawer that is open, or `null`. */
  content: Accessor<DrawerContent | null>;
  /** Replaces whatever Drawer is open. */
  open(content: DrawerContent): void;
  close(): void;
};

export const DRAWER_EASING = "ease-out";

export const createDrawer = (): DrawerState => {
  const [content, setContent] = createSignal<DrawerContent | null>(null);

  return {
    content,
    open: (next) => setContent(() => next),
    close: () => setContent(null),
  };
};
