import { createResource, createSignal, For, onCleanup, Show } from "solid-js";
import type { Pad } from "@contracts/pad/Pad";
import { ErrorLine } from "../ink/ErrorLine";
import { useConnectedProject } from "../state/connectedProject";
import { createFailure } from "./failure";
import { PadDrawer } from "./PadDrawer";
import { ownerMark, USER } from "./owner";

/** The Shelf's `pads` list; `pad.list` logs no Touch, so refetching it on every `pad.changed` leaves no trace in Provenance. */
export const Pads = () => {
  const connected = useConnectedProject();
  const [pads, { refetch }] = createResource(() => connected.app.rpc("pad.list", null));
  const [naming, setNaming] = createSignal(false);
  const failure = createFailure();

  onCleanup(
    connected.events.subscribe((event) => {
      if (event.name === "pad.changed") void refetch();
    }),
  );

  const create = (name: string) =>
    failure.run(async () => {
      setNaming(false);
      await connected.app.rpc("pad.create", { name, text: null });
    });

  const makeYours = (name: string) =>
    failure.run(async () => {
      await connected.app.rpc("pad.setOwner", { name, owner: USER });
      await refetch();
    });

  return (
    <section aria-label="pads">
      <p>
        <span class="light">pads</span>{" "}
        <button
          type="button"
          class="word"
          onClick={() => {
            failure.clear();
            setNaming(true);
          }}
        >
          +
        </button>
      </p>
      <Show when={failure.message()} fallback={<Show when={naming()}><NameField onName={create} /></Show>}>
        {(message) => <ErrorLine message={message()} />}
      </Show>
      <Show when={pads.error} fallback={<PadRows pads={pads.latest ?? []} onMakeYours={(name) => void makeYours(name)} onLateFailure={failure.show} />}>
        {(error) => <ErrorLine message={error() instanceof Error ? error().message : String(error())} />}
      </Show>
    </section>
  );
};

const PadRows = (props: { pads: readonly Pad[]; onMakeYours: (name: string) => void; onLateFailure: (message: string) => void }) => {
  const connected = useConnectedProject();

  return (
    <For each={props.pads}>
      {(pad) => (
        <p>
          <Show
            when={pad.owner.kind === "user"}
            fallback={
              <button type="button" class="word" aria-label={`make ${pad.name} yours`} onClick={() => props.onMakeYours(pad.name)}>
                {ownerMark(pad.owner)}
              </button>
            }
          >
            <span class="light">{ownerMark(pad.owner)}</span>
          </Show>{" "}
          <button type="button" class="word" onClick={() => connected.drawer.open(() => <PadDrawer name={pad.name} onLateFailure={props.onLateFailure} />)}>
            {pad.name}
          </button>
        </p>
      )}
    </For>
  );
};

const NameField = (props: { onName: (name: string) => void }) => (
  <input
    aria-label="pad name"
    ref={(field) => queueMicrotask(() => field.focus())}
    onKeyDown={(key) => {
      if (key.key === "Enter") props.onName(key.currentTarget.value);
    }}
  />
);
