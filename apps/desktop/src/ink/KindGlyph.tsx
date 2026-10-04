import type { Kind } from "@contracts/Kind";
import { glyphOf, hasInk } from "./glyph";
import { Icon } from "./Icon";

export const KindGlyph = (props: { kind: Kind; decorative?: boolean }) => (
  <span class={hasInk(glyphOf(props.kind)) ? "glyph ink" : "glyph"} data-tone={glyphOf(props.kind).tone}
    role={props.decorative ? undefined : "img"} aria-label={props.decorative ? undefined : props.kind} aria-hidden={props.decorative ? "true" : undefined}>
    <Icon name={glyphOf(props.kind).icon} filled={props.kind === "needs-you"} />
  </span>
);
