import { Icon } from "../ink/Icon";
import { Show } from "solid-js";
import { ErrorLine } from "../ink/ErrorLine";
import { Button } from "../ink/Button";

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
    <Button kind="primary" class="word" onClick={() => props.onChoose()}>
      choose folder…
    </Button>
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
