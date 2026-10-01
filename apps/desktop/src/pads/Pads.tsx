import { createSignal, For, onCleanup, Show } from "solid-js";
import type { Accessor } from "solid-js";
import type { Pad } from "@contracts/pad/Pad";
import { ErrorLine } from "../ink/ErrorLine";
import { useConnectedProject } from "../state/connectedProject";
import { createFailure } from "./failure";
import { PadDrawer } from "./PadDrawer";
import { ownerMark, USER } from "./owner";

/** The Shelf's `pads` list; `pad.list` logs no Touch, so refetching it on every `pad.changed` leaves no trace in Provenance. One fetch serves the list and any open Drawer. */
export const Pads = () => {
  const connected = useConnectedProject();
  const [pads, setPads] = createSignal<readonly Pad[]>([]);
  const [naming, setNaming] = createSignal(false);
  const failure = createFailure();
  const listFailure = createFailure();
  let newestReload = 0;

  /** Only the newest request may set the list: a slower, older reply would put stale Pads back. */
  const reload = () =>
    listFailure.run(async () => {
      newestReload += 1;

      const mine = newestReload;
      const listed = await connected.app.rpc("pad.list", null);

      if (mine === newestReload) setPads(listed);
    });

  onCleanup(
    connected.events.subscribe((event) => {
      if (event.name === "pad.changed") void reload();
    }),
  );

  void reload();

  const create = (name: string) =>
    failure.run(async () => {
      setNaming(false);
      await connected.app.rpc("pad.create", { name, text: null });
    });

  const makeYours = (name: string) =>
    failure.run(async () => {
      await connected.app.rpc("pad.setOwner", { name, owner: USER });
      await reload();
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
      <Show
        when={failure.message()}
        fallback={
          <Show when={naming()}>
            <NameField onName={create} onCancel={() => setNaming(false)} />
          </Show>
        }
      >
        {(message) => <ErrorLine message={message()} />}
      </Show>
      <Show when={listFailure.message()}>
        {(message) => <ErrorLine message={message()} />}
      </Show>
      <PadRows
        pads={pads}
        onMakeYours={(name) => void makeYours(name)}
        onLateFailure={failure.show}
      />
    </section>
  );
};

const PadRows = (props: {
  pads: Accessor<readonly Pad[]>;
  onMakeYours: (name: string) => void;
  onLateFailure: (message: string) => void;
}) => {
  const connected = useConnectedProject();

  return (
    <For each={props.pads()}>
      {(pad) => (
        <p>
          <Show
            when={pad.owner.kind === "user"}
            fallback={
              <button
                type="button"
                class="word"
                aria-label={`make ${pad.name} yours`}
                onClick={() => props.onMakeYours(pad.name)}
              >
                {ownerMark(pad.owner)}
              </button>
            }
          >
            <span class="light">{ownerMark(pad.owner)}</span>
          </Show>{" "}
          <button
            type="button"
            style={{ all: "unset", cursor: "pointer" }}
            onClick={() =>
              connected.drawer.open(() => (
                <PadDrawer
                  name={pad.name}
                  pads={props.pads}
                  onLateFailure={props.onLateFailure}
                />
              ))
            }
          >
            {pad.name}
          </button>
        </p>
      )}
    </For>
  );
};

const NameField = (props: {
  onName: (name: string) => void;
  onCancel: () => void;
}) => (
  <input
    aria-label="pad name"
    ref={(field) => queueMicrotask(() => field.focus())}
    onKeyDown={(key) => {
      if (key.key === "Enter") props.onName(key.currentTarget.value);
      else if (key.key === "Escape") props.onCancel();
    }}
  />
);
