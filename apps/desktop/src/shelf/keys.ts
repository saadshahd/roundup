import { captureFocus, queueFocus } from "../keys/focus";
import { createRoot, onCleanup } from "solid-js";
import { chordsFor } from "../keys/chords";
import { registerKeys } from "../keys/registry";
import type { ActivationSource, KeyScope } from "../keys/registry";
import type { ShelfFrame } from "./frame";

export const shelfKeyScope: KeyScope = { kind: "region", element: () => document.querySelector<HTMLElement>(".shelf") ?? undefined };

const canShelfKey = (event: KeyboardEvent) => !event.repeat && !event.isComposing && !event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey
  && event.target instanceof Element && !event.target.closest("input, textarea, select, [contenteditable], .drawer, [role=dialog], [role=alertdialog]")
  && !document.querySelector('.drawer[data-open="true"], [role=dialog], [role=alertdialog], [role=menu]');

/** U3723: both lists bind the same keys without changing their selection or pointer paths. */
export const shelfRowKeys = <Id,>(props: {
  focused: () => Id | undefined;
  remove: (id: Id) => void;
  exportPad?: (id: Id) => void;
}) => {
  registerKeys(shelfKeyScope, [
    { chords: chordsFor("D"), when: event => canShelfKey(event) && props.focused() !== undefined, run: () => {
      const id = props.focused();

      if (id !== undefined) props.remove(id);
    } },
    { chords: chordsFor("E"), when: event => canShelfKey(event) && props.exportPad !== undefined && props.focused() !== undefined, run: () => {
      const id = props.focused();

      if (id !== undefined) props.exportPad?.(id);
    } },
  ]);
};

/** U370: leader `S` toggles, `esc` from inside; focus goes to the open tab on opening and back to where it was on closing. */
export const shelfKeys = (props: { frame: ShelfFrame; shelf: () => HTMLElement | null }) => {
  let returnTo: HTMLElement | null = null;
  const tablist = () => document.querySelector<HTMLElement>(".shelf-tabs") ?? undefined;
  const selectedTab = () => tablist()?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]');
  const firstControl = () => props.shelf()?.querySelector<HTMLElement>('.shelf button:not([disabled]), .shelf [tabindex="0"], .shelf input:not([disabled])');

  const toggle = (source: ActivationSource = "pointer") => {
    const opening = !props.frame.open();
    const kept = document.activeElement;

    if (opening) {
      captureFocus(true);
      returnTo = kept instanceof HTMLElement && kept !== document.body ? kept : null;
    }

    props.frame.toggle(source);

    if (!opening && kept instanceof HTMLElement && (props.shelf()?.contains(kept) || kept.closest(".shelf-tabs"))) {
      if (returnTo?.isConnected) returnTo.focus({ preventScroll: true });
      else kept.blur();

      returnTo = null;
    }

    if (opening) queueFocus(() => selectedTab()?.focus({ preventScroll: true }));
  };

  const canEscape = (press: KeyboardEvent) => {
    if (press.defaultPrevented || press.key !== "Escape" || !props.frame.open() || !(press.target instanceof Element)) return false;

    const shelf = props.shelf();

    if (shelf?.getAttribute("data-triage") === "on") return false;

    // A naming field can remove itself while handling Escape; its original path
    // still belongs to the Shelf, so that same key must not close the fallback.
    const inside = (shelf !== null && press.composedPath().includes(shelf)) || press.target.closest(".shelf-tabs") !== null;
    const overlay = props.shelf()?.closest(".window")?.getAttribute("data-shelf-mode") === "overlay";

    if (!inside && (!overlay || !press.target.closest(".centre"))) return false;

    if ((inside && press.target.closest("input, textarea, [contenteditable]")) || document.querySelector('.drawer[data-open="true"], [role="dialog"], [role="alertdialog"], [role="menu"], [data-dnd-dragging]')) return false;

    return true;
  };

  const moveFocus = (event: KeyboardEvent) => {
    if (event.defaultPrevented || event.key !== "Tab" || !props.frame.open() || !(event.target instanceof HTMLElement)) return;

    const next = event.shiftKey
      ? event.target === firstControl() ? selectedTab() : null
      : tablist()?.contains(event.target) ? firstControl() : null;

    if (!next) return;

    event.preventDefault();
    next.focus({ preventScroll: true });
  };

  const stop = createRoot((dispose) => {
    registerKeys(shelfKeyScope, [{
      chords: ["Shift+Tab"],
      when: event => props.frame.open() && event.target === firstControl() && selectedTab() !== undefined,
      run: () => selectedTab()?.focus({ preventScroll: true }),
    }, { chords: chordsFor("M"), when: event => canShelfKey(event) && props.frame.open() && props.shelf()?.closest(".window")?.getAttribute("data-shelf-mode") !== "overlay", run: () => props.frame.toggleMaximise("key") }]);
    registerKeys({ kind: "app" }, [{ chords: chordsFor("Esc"), when: canEscape, run: () => toggle("key") }]);

    return dispose;
  });

  onCleanup(stop);

  return {
    toggle,
    moveFocus,
    stop,
  };
};
