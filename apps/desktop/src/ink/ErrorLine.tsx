import { glyphOf } from "./glyph";

/** The one line of Ink a failed call leaves on screen: `✕ <message>`. */
export const ErrorLine = (props: { message: string }) => (
  <p class="ink" data-tone={glyphOf("error").tone}>
    {glyphOf("error").mark} {props.message}
  </p>
);
