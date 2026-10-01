import { createResource, Show } from "solid-js";
import type { JSX } from "solid-js";
import { useWorkspace } from "../state/workspace";
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
  const workspace = useWorkspace();

  const [loaded] = createResource(async () => {
    const history = await workspace.app.rpc("provenance.history", { item: props.item });

    return { line: lastTouchLine(history, workspace.rail.nameOf), value: await props.read() };
  });

  return (
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
  );
};
