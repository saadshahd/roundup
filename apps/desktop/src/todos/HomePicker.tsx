import type { captureFocus } from "../keys/focus";
import { For } from "solid-js";
import type { Accessor } from "solid-js";
import { Portal } from "solid-js/web";
import { Menu } from "../ui/Menu";
import { copy } from "../copy";
import type { Home } from "./homes";

/**
 * U3753: the one list of Homes a triage row's `w` key opens below the row, as OwnerPicker does for owners.
 * `close` runs after every dismissal so the caller returns focus to that row.
 */
export const HomePicker = (props: {
  target: Accessor<{ id: number; anchor: HTMLElement; focus?: ReturnType<typeof captureFocus> } | null>;
  offers: Accessor<readonly Home[]>;
  choose: (id: number, home: string | null) => void;
  close: () => void;
}) => (
  <Menu.Root initialFocus={props.target()?.focus}
    lazyMount
    unmountOnExit
    open={props.target() !== null && props.offers().length > 0}
    positioning={{ placement: "bottom-start", getAnchorRect: () => props.target()?.anchor.getBoundingClientRect() ?? null }}
    onOpenChange={({ open }) => { if (!open) props.close(); }}
    onSelect={({ value }) => {
      const target = props.target();
      const offer = props.offers()[Number(value)];

      if (target && offer) props.choose(target.id, offer.home);
    }}
  >
    <Portal>
      <Menu.Positioner>
        <Menu.Content class="owner-picker" aria-label={copy.homesFor(props.target()?.id ?? "")}>
          <For each={props.offers()}>{(offer, index) => <Menu.Item value={String(index())}>{offer.name}</Menu.Item>}</For>
        </Menu.Content>
      </Menu.Positioner>
    </Portal>
  </Menu.Root>
);
