import { captureFocus } from "../keys/focus";
import { activationSource } from "../keys/registry";
import { registerKeys } from "../keys/registry";
import { IconButton } from "../ui/IconButton";
import { copy } from "../copy";
import { createEffect, createSignal, on, onCleanup, Show } from "solid-js";
import type { Accessor } from "solid-js";
import { Icon } from "../ink/Icon";
import { DRAWER_EASING } from "./drawer";
import { createSlideLifetime } from "../ui/slideLifetime";
import { setDrawerSliding } from "./sliding";
import type { DrawerContent, DrawerState } from "./drawer";

/**
 * An overlay: absolutely positioned over the Shelf, at the window's right edge. The centre narrows by the
 * Drawer's width with the same transition (U430), so the Drawer covers none of it (GLOSSARY.md, Drawer).
 */
export const DrawerHost = (props: { drawer: DrawerState; reducedMotion: Accessor<boolean> }) => {
  const [mounted, setMounted] = createSignal<DrawerContent | null>(null);
  const [panel, setPanel] = createSignal<HTMLElement>();
  const isOpen = () => props.drawer.content() !== null;
  const instant = () => props.reducedMotion() || props.drawer.source() === "key";
  const [settling, setSettling] = createSignal(false);
  let settleFrame: number | undefined;
  let revision = 0;
  let savedFocus: HTMLElement | null = null;

  // U40: opening takes the keyboard; closing gives it back to whatever kept it, unless focus left the Drawer on its own.
  createEffect(
    on(isOpen, (open, wasOpen) => {
      const host = panel();

      if (open && !wasOpen) {
        savedFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        captureFocus()(() => host?.querySelector<HTMLElement>(".close")?.focus({ preventScroll: true }));
      } else if (wasOpen && !open) {
        const stayedInDrawer = host?.contains(document.activeElement) ?? false;
        const toRefocus = savedFocus;

        savedFocus = null;

        if (stayedInDrawer && toRefocus?.isConnected) captureFocus()(() => toRefocus.focus({ preventScroll: true }));
      }
    }),
  );

  // U430: the centre moves for as long as the slide runs; with no motion there is no slide.
  const lifetime = createSlideLifetime();

  createEffect(() => setDrawerSliding(lifetime.sliding()));
  onCleanup(() => setDrawerSliding(false));
  createEffect(on(lifetime.exiting, (exiting, was) => { if (was && !exiting && !isOpen()) setMounted(null); }));

  createEffect(
    on(() => [props.drawer.content(), props.drawer.source(), props.reducedMotion()] as const, (_action, previous) => {
      const current = ++revision;
      cancelAnimationFrame(settleFrame ?? 0);
      setSettling(previous !== undefined && instant());
      const content = props.drawer.content();

      if (content !== null) setMounted(() => content);
      else if (instant()) setMounted(null);

      if (previous === undefined) return;

      // U3010b: a preference change alone settles; turning reduced motion off replays no slide.
      const preferenceOnly = previous[0] === props.drawer.content() && previous[1] === props.drawer.source();

      lifetime.retarget(isOpen(), !instant() && !preferenceOnly);

      if (!instant()) return;

      // Flush the settled layout before restoring the Shelf's own transition policy.
      queueMicrotask(() => {
        if (current !== revision || !panel()?.isConnected) return;
        void panel()!.offsetWidth;
        settleFrame = requestAnimationFrame(() => setSettling(false));
      });
    }),
  );

  onCleanup(() => { revision++; cancelAnimationFrame(settleFrame ?? 0); });

  registerKeys({ kind: "layer", element: panel, active: isOpen, modal: false }, [{
    chords: ["Escape"],
    when: (press) => !(press.target instanceof Element && press.target.closest('input, textarea, [contenteditable], .xterm, .pane-screen')),
    run: () => { captureFocus(true); props.drawer.close("key"); },
  }]);

  return (
    <aside
      class="drawer"
      aria-label={copy.drawer}
      inert={!isOpen()}
      data-open={isOpen()}
      data-motion={instant() ? "none" : "slide"}
      data-settling={settling()}
      ref={setPanel}
      style={{
        position: "absolute",
        top: "0",
        right: "0",
        bottom: "0",
        transform: isOpen() ? "translateX(0)" : "translateX(100%)",
        transition: instant() ? "none" : `transform var(--duration-drawer) ${DRAWER_EASING}`,
      }}
      onTransitionEnd={(event) => {
        if (event.target !== event.currentTarget || (event.propertyName && event.propertyName !== "transform")) return;
        lifetime.settle();
      }}
    >
      <IconButton class="close" aria-label={copy.close} onClick={(event) => props.drawer.close(activationSource(event))}>
        <Icon name="x" />
      </IconButton>
      <Show when={mounted()} keyed>
        {(content) => content()}
      </Show>
    </aside>
  );
};
