import { captureFocus } from "../keys/focus";
import { onMount, type JSX } from "solid-js";
import { copy } from "../copy";
import "./styles.css";

/** U3764: the centre's settings view, shared by Project settings and App Settings: a title, a close control and named groups. Escape closes it. */
export const SettingsShell = (props: { heading: string; onClose: () => void; children: JSX.Element }) => {
  let view!: HTMLElement;

  // Opening moves focus into the view, so Escape reaches its handler.
  const initialFocus = captureFocus();
  onMount(() => initialFocus(() => view.focus()));

  return (
  <section ref={view} tabindex="-1" class="settings-view" aria-label={props.heading} onKeyDown={event => { if (event.key === "Escape" && !event.isComposing && !event.defaultPrevented) { event.preventDefault(); props.onClose(); } }}>
    <header class="settings-head">
      <h2 class="settings-title">{props.heading}</h2>
      <button data-button="quiet" class="word" type="button" onClick={() => props.onClose()}>{copy.close}</button>
    </header>
    {props.children}
  </section>
  );
};

export const SettingsGroup = (props: { name: string; children: JSX.Element }) => (
  <section class="settings-group" aria-label={props.name}>
    <h3 class="settings-group-name">{props.name}</h3>
    {props.children}
  </section>
);
