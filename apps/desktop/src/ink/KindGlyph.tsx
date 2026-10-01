import type { Kind } from "@contracts/Kind";
import { glyphOf, hasInk } from "./glyph";

/** The mark for a Kind; Ink (bold plus colour) only for `needs-you` and `error`. Colours live in `styles.css` under `[data-tone]`. */
export const KindGlyph = (props: { kind: Kind }) => (
  <span class={hasInk(glyphOf(props.kind)) ? "glyph ink" : "glyph"} data-tone={glyphOf(props.kind).tone}>
    {glyphOf(props.kind).mark}
  </span>
);
