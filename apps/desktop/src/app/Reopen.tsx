import "./reopen.styles.css";

/**
 * `reopen` after the Daemon's exit text (U37): asks the App for a new Daemon for the same Project.
 * Its word is drawn from `data-label`, so the header's text stays the exit text `u25_` pins; the name is `aria-label`.
 */
export const Reopen = (props: { onReopen: () => void }) => (
  <button class="word reopen" data-label="reopen" aria-label="reopen" onClick={props.onReopen} />
);
