import { createSignal, For, onCleanup, Show } from "solid-js";
import { CHORDS } from "./chords";
import { placeOf } from "./places";

const inTextField = (target: Element): boolean => target.closest('input, textarea, select, [contenteditable="true"]') !== null;

/** `?` opens Help from the Rail or the pane's frame, never from a text field, a Terminal, a Drawer or the switcher (U54). */
const opensFrom = (target: EventTarget | null): boolean => {
  if (!(target instanceof Element) || inTextField(target) || target.closest(".pane-screen")) return false;

  const place = placeOf(target);

  return place === "rail" || place === "pane";
};

/** The panel `?` opens over the centre pane, one line per chord. It holds focus while open; Esc, `?` or a click outside closes it and focus returns to where it was. */
export const Help = () => {
  const [open, setOpen] = createSignal(false);
  let panel: HTMLElement | undefined;
  let returnTo: Element | null = null;

  const show = (): void => {
    returnTo = document.activeElement;
    setOpen(true);
    panel?.focus();
  };

  const hide = (): void => {
    setOpen(false);

    if (returnTo instanceof HTMLElement && returnTo.isConnected) returnTo.focus();

    returnTo = null;
  };

  const onKey = (press: KeyboardEvent): void => {
    if (open()) {
      // Window capture runs before every other listener: while Help is open no key reaches a Terminal or a chord.
      press.stopPropagation();

      if (press.key === "Escape" || press.key === "?") {
        press.preventDefault();
        hide();
      } else if (press.key === "Tab") press.preventDefault();

      return;
    }

    if (press.key !== "?" || press.metaKey || press.ctrlKey || press.altKey || !opensFrom(press.target)) return;

    press.preventDefault();
    show();
  };

  const onClick = (click: MouseEvent): void => {
    if (open() && !(click.target instanceof Node && panel?.contains(click.target))) hide();
  };

  window.addEventListener("keydown", onKey, true);
  document.addEventListener("click", onClick, true);
  onCleanup(() => {
    window.removeEventListener("keydown", onKey, true);
    document.removeEventListener("click", onClick, true);
  });

  return (
    <Show when={open()}>
      <section class="help" role="dialog" aria-label="help" tabIndex={-1} ref={(element) => (panel = element)}>
        <For each={CHORDS}>
          {(chord) => (
            <p class="help-line">
              <span class="help-keys">{chord.keys}</span>
              {`  ${chord.what}`}
            </p>
          )}
        </For>
      </section>
    </Show>
  );
};
