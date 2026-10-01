import type { JSX } from "solid-js";

/** A clickable row of the Shelf or a Drawer: plain text, no button chrome; a long title wraps under its `#`, not under the glyph. */
export const RowButton = (props: { onClick: () => void; children: JSX.Element }) => (
  <button
    type="button"
    onClick={() => props.onClick()}
    style={{ all: "unset", display: "block", cursor: "pointer", "white-space": "pre-wrap", "padding-left": "2ch", "text-indent": "-2ch" }}
  >
    {props.children}
  </button>
);
