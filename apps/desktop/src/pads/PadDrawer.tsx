import { createEffect, createSignal, on, onCleanup, Show } from "solid-js";
import type { Accessor } from "solid-js";
import type { Actor } from "@contracts/Actor";
import type { Pad } from "@contracts/pad/Pad";
import { ItemDrawer } from "../drawer/ItemDrawer";
import { ErrorLine } from "../ink/ErrorLine";
import { useConnectedProject } from "../state/connectedProject";
import { createFailure } from "./failure";
import { cutName, markedLine, wholeWord } from "./nameLine";
import { ownerMark } from "./owner";

const PadBody = (props: {
  initial: Pad;
  pads: Accessor<readonly Pad[]>;
  onLateFailure: (message: string) => void;
}) => {
  const connected = useConnectedProject();
  const name = props.initial.name;
  const [pad, setPad] = createSignal(props.initial);
  const [shown, setShown] = createSignal(props.initial.text);
  const [origin, setOrigin] = createSignal(props.initial.text);
  const [editing, setEditing] = createSignal(false);
  const [conflictActor, setConflictActor] = createSignal<Actor | null>(null);
  const actionFailure = createFailure();
  const [deleted, setDeleted] = createSignal(false);
  let textField: HTMLTextAreaElement | undefined;
  let pendingActor: Actor | null = null;
  let closed = false;

  /** Moves the field and the dirty/clean baseline forward together and drops any conflict line; skip either and a resolved change still looks unresolved. The field is written directly because `shown()` can already equal `next.text` while the user's keystrokes left the real DOM value stale, and Solid skips the write when the signal itself does not change. */
  const settle = (next: Pad) => {
    setOrigin(next.text);
    setShown(next.text);

    if (textField) textField.value = next.text;

    setConflictActor(null);
  };

  /** The baseline always moves to the last loaded or saved text, even mid-edit, so a later dirty-check stays correct; the field itself only follows while the user is not typing, or their keystrokes would vanish. */
  const adopt = (next: Pad) => {
    setPad(next);
    setOrigin(next.text);

    if (!editing()) settle(next);
  };

  const sameContent = (left: Pad, right: Pad) =>
    left.text === right.text &&
    left.owner.kind === right.owner.kind &&
    left.owner.id === right.owner.id;

  onCleanup(
    connected.events.subscribe((event) => {
      if (event.name === "pad.changed" && event.data.name === name) pendingActor = event.actor;
    }),
  );

  createEffect(
    on(
      props.pads,
      (listed) => {
        const current = listed.find((entry) => entry.name === name);

        setDeleted(current === undefined);

        const actor = pendingActor;
        pendingActor = null;

        if (!current) {
          setConflictActor(null);

          return;
        }

        if (sameContent(current, pad())) return;

        setPad(current);

        const typed = textField?.value ?? shown();

        if (typed === origin()) {
          settle(current);

          return;
        }

        if (actor && actor.kind !== "user") setConflictActor(actor);
      },
      { defer: true },
    ),
  );

  onCleanup(() => {
    closed = true;
  });

  /** A call that fails after the Drawer closed has no ✕ line to land on, so it goes to the Shelf's. */
  const act = (work: () => Promise<void>) =>
    actionFailure.run(async () => {
      try {
        await work();
      } catch (thrown) {
        if (!closed || !(thrown instanceof Error)) throw thrown;

        props.onLateFailure(thrown.message);
      }
    });

  const ownedByUser = () => pad().owner.kind === "user";

  const write = (text: string) =>
    act(async () => {
      if (text !== origin())
        adopt(await connected.app.rpc("pad.write", { name, text }));
    });

  const append = (text: string) =>
    act(async () => {
      adopt(await connected.app.rpc("pad.append", { name, text }));
    });

  const exportToFile = () =>
    act(async () => {
      const path = await connected.app.chooseSavePath(`${name}.md`);

      if (path !== null) await connected.app.rpc("pad.export", { name, path });
    });

  const leaveField = async (typed: string) => {
    if (!editing()) return;

    setEditing(false);

    if (conflictActor()) return;

    await write(typed);
  };

  const keepMine = () =>
    act(async () => {
      const typed = textField?.value ?? shown();

      adopt(await connected.app.rpc("pad.write", { name, text: typed }));
      // adopt alone won't clear this while a quick refocus keeps the field dirty.
      setConflictActor(null);
    });

  const useTheirs = () => settle(pad());

  return (
    <>
      <p style={markedLine}>
        <span style={wholeWord}>{ownerMark(pad().owner)}</span>{" "}
        <span title={name} style={cutName}>
          {name}
        </span>{" "}
        <span class="light" style={wholeWord}>
          owned by {connected.rail.nameOf(pad().owner)}
        </span>
      </p>
      <p style={{ "text-align": "right" }}>
        <button
          type="button"
          class="light"
          style={{ all: "unset", cursor: "pointer", color: "var(--light)" }}
          onClick={() => void exportToFile()}
        >
          export .md
        </button>
      </p>
      <Show when={actionFailure.message()}>
        {(message) => <ErrorLine message={message()} />}
      </Show>
      <Show when={deleted()}>
        <ErrorLine message={`pad ${name} was deleted`} />
      </Show>
      <textarea
        ref={(element) => (textField = element)}
        aria-label="text"
        style={{
          "font-family": "var(--mono)",
          width: "100%",
          "min-height": "16em",
        }}
        readOnly={!ownedByUser()}
        value={shown()}
        onInput={(typed) => {
          setEditing(ownedByUser());

          if (typed.currentTarget.value === origin()) settle(pad());
        }}
        onBlur={(blurred) => void leaveField(blurred.currentTarget.value)}
      />
      <Show when={conflictActor()}>
        {(actor) => (
          <p style={markedLine}>
            <span class="light" style={wholeWord}>
              changed by {connected.rail.nameOf(actor())}
            </span>
            <button
              type="button"
              class="word"
              style={wholeWord}
              onClick={() => void keepMine()}
            >
              keep mine
            </button>
            <button
              type="button"
              class="word"
              style={wholeWord}
              onClick={useTheirs}
            >
              use theirs
            </button>
          </p>
        )}
      </Show>
      <Show when={!ownedByUser()}>
        <input
          aria-label="append"
          placeholder="append"
          onKeyDown={(key) => {
            const field = key.currentTarget;

            if (key.key !== "Enter" || field.value === "") return;

            const text = field.value;
            field.value = "";
            void append(text).then(() => {
              if (actionFailure.message() !== null) field.value = text;
            });
          }}
        />
      </Show>
    </>
  );
};

/** Body of a Pad's Drawer: the Pad is read once, as the user's Touch, after the history that last-touch line shows. */
export const PadDrawer = (props: {
  name: string;
  pads: Accessor<readonly Pad[]>;
  onLateFailure: (message: string) => void;
}) => {
  const connected = useConnectedProject();

  return (
    <ItemDrawer
      item={`pad:${props.name}`}
      read={() => connected.app.rpc("pad.read", { name: props.name })}
    >
      {(pad) => (
        <PadBody
          initial={pad}
          pads={props.pads}
          onLateFailure={props.onLateFailure}
        />
      )}
    </ItemDrawer>
  );
};
