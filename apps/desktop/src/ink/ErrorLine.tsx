import { Icon } from "./Icon";

export const ErrorLine = (props: { message: string }) => (
  <p class="ink" data-tone="red" role="alert" title={props.message}>
    <Icon name="x" />{props.message.split(/\r\n|[\r\n]/, 1)[0]}
  </p>
);

/** For `ErrorBoundary`: Solid hands a fallback an Error even when something else was thrown (a string becomes its message; anything else reads "Unknown error", with the original as `cause`). */
export const failureLine = (failure: Error) => <ErrorLine message={failure.message} />;
