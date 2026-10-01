import { createEffect, createSignal, Show } from "solid-js";
import type { Accessor } from "solid-js";
import { DRAWER_EASING, DRAWER_SLIDE_MS } from "./drawer";
import type { DrawerContent, DrawerState } from "./drawer";

/**
 * An overlay: absolutely positioned over the centre and the Shelf, so opening it moves no
 * other box and the terminal pane keeps its size (CONTEXT.md, Drawer).
 */
export const DrawerHost = (props: { drawer: DrawerState; reducedMotion: Accessor<boolean> }) => {
  const [mounted, setMounted] = createSignal<DrawerContent | null>(null);
  const isOpen = () => props.drawer.content() !== null;

  // Content stays mounted until the slide-out ends; with no transition no `transitionend` fires, so it goes at once.
  createEffect(() => {
    const content = props.drawer.content();

    if (content !== null) setMounted(() => content);
    else if (props.reducedMotion()) setMounted(null);
  });

  return (
    <aside
      class="drawer"
      aria-label="drawer"
      inert={!isOpen()}
      style={{
        position: "absolute",
        top: "0",
        right: "0",
        bottom: "0",
        transform: isOpen() ? "translateX(0)" : "translateX(100%)",
        transition: props.reducedMotion() ? "none" : `transform ${DRAWER_SLIDE_MS}ms ${DRAWER_EASING}`,
      }}
      onTransitionEnd={() => {
        if (!isOpen()) setMounted(null);
      }}
    >
      <button type="button" class="word close" onClick={() => props.drawer.close()}>
        close
      </button>
      <Show when={mounted()} keyed>
        {(content) => content()}
      </Show>
    </aside>
  );
};
