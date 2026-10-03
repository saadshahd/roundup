import { createEffect, createSignal, lazy, on, onCleanup, Show, Suspense } from "solid-js";
import type { Accessor } from "solid-js";
import type { Actor } from "@contracts/Actor";
import type { Pad } from "@contracts/pad/Pad";
import { ItemDrawer } from "../drawer/ItemDrawer";
import { ErrorLine } from "../ink/ErrorLine";
import { useConnectedProject } from "../state/connectedProject";
import { createFailure } from "./failure";
import { cutName, markedLine, wholeWord } from "./nameLine";
import { ownerMark } from "./owner";
import type { PadEditorHandle } from "./PadEditor";
import "./styles.css";

const PadMarkdown = lazy(() => import("./PadMarkdown"));

const PadEditor = lazy(() => import("./PadEditor"));

const PadRichEditor = lazy(() => import("./PadRichEditor"));

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
  const [mode, setMode] = createSignal<"read" | "edit" | "source" | "preview">("read");
  const [conflictActor, setConflictActor] = createSignal<Actor | null>(null);
  const actionFailure = createFailure();
  const [deleted, setDeleted] = createSignal(false);
  let editor: PadEditorHandle | undefined;
  let reader: HTMLDivElement | undefined;
  let pendingActor: Actor | null = null;
  let closed = false;
  let switchingView = false;

  /** Moves the source and its dirty/clean baseline together so a resolved change cannot remain a conflict. */
  const settle = (next: Pad) => {
    setOrigin(next.text);
    setShown(next.text);
    editor?.setText(next.text);
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

  const currentDraft = () => editor?.getText() ?? shown();

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

        if (currentDraft() === origin()) {
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
      queueMicrotask(() => {
        if (reader) reader.scrollTop = reader.scrollHeight;
      });
    });

  const exportToFile = () =>
    act(async () => {
      const path = await connected.app.chooseSavePath(`${name}.md`);

      if (path !== null) await connected.app.rpc("pad.export", { name, path });
    });

  const leaveField = async (typed: string) => {
    if (!editing()) return;

    if (conflictActor()) return;

    setEditing(false);
    await write(typed);
  };

  const onText = (text: string) => {
    setShown(text);
    setEditing(ownedByUser());

    if (text === origin()) settle(pad());
  };

  const onEditorBlur = (next: EventTarget | null) => {
    if (switchingView) return;

    if (next instanceof Node && next.parentElement?.closest(".pad-actions")) return;

    if (next instanceof Node && next.parentElement?.closest(".pad-rich-editor")) return;

    void leaveField(currentDraft());
  };

  const done = () => {
    if (conflictActor()) return;

    switchingView = true;
    void leaveField(currentDraft());
    editor = undefined;
    setMode("read");
  };

  const keepMine = () =>
    act(async () => {
      adopt(await connected.app.rpc("pad.write", { name, text: currentDraft() }));
      // adopt alone won't clear this while a quick refocus keeps the field dirty.
      setConflictActor(null);
    });

  const useTheirs = () => settle(pad());

  return (
    <div class="pad-body">
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
          style={{ all: "unset", cursor: "pointer", color: "var(--grey)" }}
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
      <Show when={ownedByUser()}>
        <div class="pad-actions">
          <Show when={mode() === "read"}>
            <button type="button" onClick={() => setMode("edit")}>edit</button>
          </Show>
          <Show when={mode() === "edit"}>
            <button type="button" onClick={() => {
              switchingView = true;
              editor = undefined;
              setMode("source");
            }}>source</button>
            <button
              type="button"
              onClick={() => {
                switchingView = true;
                editor = undefined;
                setMode("preview");
              }}
            >
              Preview
            </button>
          </Show>
          <Show when={mode() === "source"}>
            <button type="button" onClick={() => {
              switchingView = true;
              editor = undefined;
              setMode("edit");
            }}>formatted</button>
            <button type="button" onClick={() => {
              switchingView = true;
              editor = undefined;
              setMode("preview");
            }}>Preview</button>
          </Show>
          <Show when={mode() === "preview"}>
            <button type="button" onClick={() => setMode("edit")}>edit</button>
          </Show>
          <Show when={mode() !== "read"}>
            <button type="button" onClick={done}>done</button>
          </Show>
        </div>
      </Show>
      <Suspense>
        <Show when={mode() === "edit" && ownedByUser()}>
          <PadRichEditor
            text={shown()}
            onText={onText}
            onBlur={onEditorBlur}
            onReady={(ready) => {
              editor = ready;
              switchingView = false;
            }}
          />
        </Show>
        <Show when={mode() === "source" && ownedByUser()}>
          <PadEditor
            text={shown()}
            onText={onText}
            onBlur={onEditorBlur}
            onReady={(ready) => {
              editor = ready;
              switchingView = false;
            }}
          />
        </Show>
        <Show when={mode() === "preview" && ownedByUser()}>
          <PadMarkdown text={shown()} label="Preview" />
        </Show>
        <Show when={mode() === "read" || !ownedByUser()}>
          <PadMarkdown
            text={shown()}
            label="Pad body"
            onReader={(element) => (reader = element)}
          />
        </Show>
      </Suspense>
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
    </div>
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
