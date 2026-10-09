import { createResource, ErrorBoundary, Show } from "solid-js";
import type { JSX } from "solid-js";
import { failureLine } from "../ink/ErrorLine";
import { useConnectedProject } from "../state/connectedProject";
import { lastTouchLine } from "./touchLine";

/**
 * Body of a Drawer for `todo:<id>` or `pad:<name>`. It fetches `provenance.history` first and only then calls
 * `read`: the read is itself a Touch by the user, and fetching after it would show "you read" at the moment of opening.
 */
export const ItemDrawer = <Value,>(props: {
  item: string;
  read: () => Promise<Value>;
  children: (value: Value) => JSX.Element;
}) => {
  const connected = useConnectedProject();

  const [loaded] = createResource(async () => {
    const history = await connected.app.rpc("provenance.history", { item: props.item });

    return { line: lastTouchLine(history, connected.rail.nameOf), value: await props.read() };
  });

  return (
    <ErrorBoundary fallback={failureLine}>
      <Show when={loaded()}>
        {(item) => (
          <>
            {props.children(item().value)}
            <Show when={item().line}>
              {(line) => (
                <p class="light" style={{ "white-space": "pre" }}>
                  {line()}
                </p>
              )}
            </Show>
          </>
        )}
      </Show>
    </ErrorBoundary>
  );
};
