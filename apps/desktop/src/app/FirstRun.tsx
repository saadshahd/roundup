import { Icon } from "../ink/Icon";
import { Show } from "solid-js";
import { ErrorLine } from "../ink/ErrorLine";

/** Screen 11 of `docs/wireframes.md`: each region says what will live there; there is no tutorial. */
export const EmptyRail = () => (
  <>
    <p>agents and terminals</p>
    <p>appear here, one per row,</p>
    <p>nested by indent</p>
  </>
);

export const ChooseProject = (props: { failure: string | null; onChoose: () => void }) => (
  <>
    <Show when={props.failure} fallback={<p>open a folder to start</p>}>
      {(message) => <ErrorLine message={message()} />}
    </Show>
    <button type="button" class="word" onClick={() => props.onChoose()}>
      choose folder…
    </button>
  </>
);

export const EmptyShelf = () => (
  <>
    <p>todos</p>
    <p>agents add them as they</p>
    <p>plan; so can you</p>
    <p>pads</p>
    <p><Icon name="owned" /> agent notes</p>
    <p><Icon name="diamond" /> yours</p>
  </>
);
