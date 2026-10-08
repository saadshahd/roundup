import { createMemo, createSignal, For, Show } from "solid-js";
import type { Accessor } from "solid-js";
import type { RailNode } from "@contracts/agent/RailNode";
import { ErrorLine } from "../../ink/ErrorLine";
import { Icon } from "../../ink/Icon";
import { OwnerMark } from "../../ink/OwnerMark";
import { PadDrawer } from "../../pads/PadDrawer";
import { cutName } from "../../pads/nameLine";
import { useConnectedProject } from "../../state/connectedProject";
import { padsOwnedBy } from "./owned";
import { usePadList } from "./padList";

/** The Pads under one Rail row (U58): `n` is how many its Agent owns (0 for any other row), `shown` is whether the control is open. */
export const createAgentPads = (node: Accessor<RailNode>) => {
  const all = usePadList().pads;
  const [shown, setShown] = createSignal(false);
  const [failure, setFailure] = createSignal<string | null>(null);
  const owned = createMemo(() => (node().kind === "agent" ? padsOwnedBy(all(), node().id) : []));

  return {
    all,
    owned,
    n: () => owned().length,
    shown: () => shown() && owned().length > 0,
    toggle: () => setShown((open) => !open),
    failure,
    setFailure,
  };
};

export type AgentPads = ReturnType<typeof createAgentPads>;

/** `◈ <n>` at the right end of an Agent's row; absent at 0. */
export const PadsControl = (props: { pads: AgentPads }) => (
  <Show when={props.pads.n() > 0}>
    <button
      class="word rail-pads-control"
      data-pads-control
      aria-label={`${props.pads.n()} pads`}
      aria-expanded={props.pads.shown()}
      onClick={(click) => {
        click.stopPropagation();
        props.pads.toggle();
      }}
    >
      <Icon name="owned" /> {props.pads.n()}
    </button>
  </Show>
);

/** The Pad rows one level below `depth`, folded at rest; a click opens a Pad's Drawer as the Shelf's list does (U20). Opening changes no selection (U64). */
export const PadRows = (props: { pads: AgentPads; depth: number }) => {
  const connected = useConnectedProject();

  return (
    <>
      <Show when={props.pads.shown()}>
        <div class="rail-pads" data-pads role="group" style={{ "padding-left": `${(props.depth + 1) * 2}ch` }}>
          <For each={props.pads.owned()}>
            {(pad) => (
              <button
                type="button"
                class="rail-pad"
                title={pad.name}
                onClick={() => {
                  props.pads.setFailure(null);
                  connected.drawer.open(() => (
                    <PadDrawer name={pad.name} pads={props.pads.all} onLateFailure={props.pads.setFailure} />
                  ));
                }}
              >
                <OwnerMark owner={pad.owner} decorative />
                <span style={cutName}>{pad.name}</span>
              </button>
            )}
          </For>
        </div>
      </Show>
      <Show when={props.pads.failure()}>{(message) => <ErrorLine message={message()} />}</Show>
    </>
  );
};
