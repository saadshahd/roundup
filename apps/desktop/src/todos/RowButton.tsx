import type { JSX } from "solid-js";
import "./rowButton.styles.css";

/** A clickable row of the Shelf or a Drawer: plain text, no button chrome; a long title wraps under its `#`, not under the glyph. */
export const RowButton = (props: { onClick: () => void; children: JSX.Element; ref?: (row: HTMLButtonElement) => void }) => (
  <button
    type="button"
    class="todo-row-button"
    data-row
    ref={props.ref}
    onClick={() => props.onClick()}
    style={{ display: "block", "min-height": "var(--space-5)", cursor: "pointer", "white-space": "pre-wrap", "padding-left": "var(--todo-row-indent)", "text-indent": "calc(-1 * var(--todo-row-indent))" }}
  >
    {props.children}
  </button>
);
