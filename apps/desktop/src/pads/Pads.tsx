import { OwnerMark } from "../ink/OwnerMark";
import { Icon } from "../ink/Icon";
import { createMemo, createSignal, For, onCleanup, Show } from "solid-js";
import type { Accessor } from "solid-js";
import type { Pad } from "@contracts/pad/Pad";
import { ErrorLine } from "../ink/ErrorLine";
import { useConnectedProject } from "../state/connectedProject";
import { createFailure } from "./failure";
import { PadDrawer } from "./PadDrawer";
import { cutName, markedLine, wholeWord } from "./nameLine";
import { USER } from "./owner";

/** The Shelf's `pads` list; `pad.list` logs no Touch, so refetching it on every `pad.changed` leaves no trace in Provenance. One fetch serves the list and any open Drawer. */
export const Pads = () => {
  const connected = useConnectedProject();
  const [pads, setPads] = createSignal<readonly Pad[]>([]);
  const [naming, setNaming] = createSignal(false);
  const failure = createFailure();
  const listFailure = createFailure();
  let newestReload = 0;
  /** True once the newest `pad.list` has answered, so U38's empty line never flashes before it. */
  const [isLoaded, setIsLoaded] = createSignal(false);

  /** Only the newest request may set the list, or its failure, or `isLoaded`: a slower, older reply must not put stale Pads back, overwrite a later failure, or flash the empty line while a retry is still in flight. */
  const reload = async () => {
    newestReload += 1;

    const mine = newestReload;

    try {
      const listed = await connected.app.rpc("pad.list", null);

      if (mine !== newestReload) return;

      setPads(listed);
      listFailure.clear();
    } catch (thrown) {
      if (!(thrown instanceof Error)) throw thrown;

      if (mine !== newestReload) return;

      listFailure.show(thrown.message);
    }

    setIsLoaded(true);
  };

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

  const isEmpty = createMemo(() => isLoaded() && pads().length === 0 && listFailure.message() === null);

  return (
    <section aria-label="pads">
      <p>
        <span class="light">pads</span>{" "}
        <button
          type="button"
          aria-label="add pad"
          class="word"
          onClick={() => {
            failure.clear();
            setNaming(true);
          }}
        >
          <Icon name="plus" />
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
      <Show when={isEmpty()}>
        <p>no pads yet</p>
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
        <p style={markedLine}>
          <Show
            when={pad.owner.kind === "user"}
            fallback={
              <button
                type="button"
                class="word"
                style={wholeWord}
                aria-label={`make ${pad.name} yours`}
                onClick={() => props.onMakeYours(pad.name)}
              >
                <OwnerMark owner={pad.owner} />
              </button>
            }
          >
            <span class="light" style={wholeWord}>
              <OwnerMark owner={pad.owner} />
            </span>
          </Show>
          <button
            type="button"
            title={pad.name}
            class="pad-name"
            style={cutName}
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
