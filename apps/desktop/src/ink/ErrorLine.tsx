import { glyphOf } from "./glyph";

/** The one line of Ink a failed call leaves on screen: `✕ <message>`. */
export const ErrorLine = (props: { message: string }) => (
  <p class="ink" data-tone={glyphOf("error").tone}>
    {glyphOf("error").mark} {props.message}
  </p>
);

/** For `ErrorBoundary`: a thrown Error becomes its `✕` line; anything else is a bug and escapes. */
export const failureLine = (failure: Error) => {
  if (!(failure instanceof Error)) throw failure;

  return <ErrorLine message={failure.message} />;
};
