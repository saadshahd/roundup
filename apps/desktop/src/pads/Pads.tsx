import { createResource, createSignal, For, onCleanup, Show } from "solid-js";
import type { Pad } from "@contracts/pad/Pad";
import { ErrorLine } from "../ink/ErrorLine";
import { useWorkspace } from "../state/workspace";
import { createFailure } from "./failure";
import { PadDrawer } from "./PadDrawer";
import { ownerMark, USER } from "./owner";

/** The Shelf's `pads` list; `pad.list` logs no Touch, so refetching it on every `pad.changed` leaves no trace in Provenance. */
export const Pads = () => {
  const workspace = useWorkspace();
  const [pads, { refetch }] = createResource(() => workspace.app.rpc("pad.list", null));
  const [naming, setNaming] = createSignal(false);
  const failure = createFailure();

  onCleanup(
    workspace.events.subscribe((event) => {
      if (event.name === "pad.changed") void refetch();
    }),
  );

  const create = (name: string) =>
    failure.run(async () => {
      setNaming(false);
      await workspace.app.rpc("pad.create", { name, text: null });
    });

  const makeYours = (name: string) =>
    failure.run(async () => {
      await workspace.app.rpc("pad.setOwner", { name, owner: USER });
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
      <Show when={pads.error} fallback={<PadRows pads={pads.latest ?? []} onMakeYours={(name) => void makeYours(name)} />}>
        {(error) => <ErrorLine message={error() instanceof Error ? error().message : String(error())} />}
      </Show>
    </section>
  );
};

const PadRows = (props: { pads: readonly Pad[]; onMakeYours: (name: string) => void }) => {
  const workspace = useWorkspace();

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
          <button type="button" class="word" onClick={() => workspace.drawer.open(() => <PadDrawer name={pad.name} />)}>
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
