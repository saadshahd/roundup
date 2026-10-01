import { createResource, Show } from "solid-js";
import { useWorkspace } from "../state/workspace";
import { lastTouchLine } from "./touchLine";

/** The last-touch line of a Drawer for `todo:<id>` or `pad:<name>`. `provenance.history` logs no Touch itself. */
export const LastTouch = (props: { item: string }) => {
  const workspace = useWorkspace();

  const [history] = createResource(
    () => props.item,
    (item) => workspace.app.rpc("provenance.history", { item }),
  );

  return (
    <Show when={history()}>
      {(touches) => (
        <p class="light" style={{ "white-space": "pre" }}>
          {lastTouchLine(touches(), workspace.rail.nameOf)}
        </p>
      )}
    </Show>
  );
};
