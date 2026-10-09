import { createEffect, createSignal, on, onCleanup, Show } from "solid-js";
import type { Accessor } from "solid-js";
import { Icon } from "../ink/Icon";
import { DRAWER_EASING } from "./drawer";
import type { DrawerContent, DrawerState } from "./drawer";

/**
 * An overlay: absolutely positioned over the centre and the Shelf, so opening it moves no
 * other box and the terminal pane keeps its size (GLOSSARY.md, Drawer).
 */
export const DrawerHost = (props: { drawer: DrawerState; reducedMotion: Accessor<boolean> }) => {
  const [mounted, setMounted] = createSignal<DrawerContent | null>(null);
  const [panel, setPanel] = createSignal<HTMLElement>();
  const isOpen = () => props.drawer.content() !== null;
  let heldFocus: HTMLElement | null = null;

  // Content stays mounted until the slide-out ends; with no transition no `transitionend` fires, so it goes at once.
  createEffect(() => {
    const content = props.drawer.content();

    if (content !== null) setMounted(() => content);
    else if (props.reducedMotion()) setMounted(null);
  });

  // U40: opening takes the keyboard; closing gives it back to whatever held it, unless focus left the Drawer on its own.
  createEffect(
    on(isOpen, (open, wasOpen) => {
      const host = panel();

      if (open && !wasOpen) {
        heldFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        host?.querySelector<HTMLElement>(".close")?.focus({ preventScroll: true });
      } else if (wasOpen && !open) {
        const stayedInDrawer = host?.contains(document.activeElement) ?? false;
        const toRefocus = heldFocus;

        heldFocus = null;

        if (stayedInDrawer) toRefocus?.focus({ preventScroll: true });
      }
    }),
  );

  // A field or the terminal owns Esc: it cancels the field or reaches the program.
  const closeOnEsc = (key: KeyboardEvent) => {
    const editing = key.target instanceof HTMLElement && key.target.matches("input, textarea, [contenteditable]");

    if (key.key === "Escape" && isOpen() && !editing) props.drawer.close();
  };

  document.addEventListener("keydown", closeOnEsc);
  onCleanup(() => document.removeEventListener("keydown", closeOnEsc));

  return (
    <aside
      class="drawer"
      aria-label="drawer"
      inert={!isOpen()}
      data-open={isOpen()}
      ref={setPanel}
      style={{
        position: "absolute",
        top: "0",
        right: "0",
        bottom: "0",
        transform: isOpen() ? "translateX(0)" : "translateX(100%)",
        transition: props.reducedMotion() ? "none" : `transform var(--duration-drawer) ${DRAWER_EASING}`,
      }}
      onTransitionEnd={() => {
        if (!isOpen()) setMounted(null);
      }}
    >
      <button type="button" class="word close" aria-label="close" onClick={() => props.drawer.close()}>
        <Icon name="x" />
      </button>
      <Show when={mounted()} keyed>
        {(content) => content()}
      </Show>
    </aside>
  );
};
