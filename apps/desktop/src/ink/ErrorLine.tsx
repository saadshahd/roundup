import { glyphOf } from "./glyph";

/** The one line of Ink a failed call leaves on screen: `✕ <message>`. */
export const ErrorLine = (props: { message: string }) => (
  <p class="ink" data-tone={glyphOf("error").tone}>
    {glyphOf("error").mark} {props.message}
  </p>
);

/** For `ErrorBoundary`: Solid hands a fallback an Error even when something else was thrown (a string becomes its message; anything else reads "Unknown error", with the original as `cause`). */
export const failureLine = (failure: Error) => <ErrorLine message={failure.message} />;
