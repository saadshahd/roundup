import { createSignal, onCleanup, Show } from "solid-js";
import type { Pad } from "@contracts/pad/Pad";
import { ItemDrawer } from "../drawer/ItemDrawer";
import { ErrorLine } from "../ink/ErrorLine";
import { useConnectedProject } from "../state/connectedProject";
import { createFailure } from "./failure";
import { ownerMark } from "./owner";

const PadBody = (props: { initial: Pad; onLateFailure: (message: string) => void }) => {
  const connected = useConnectedProject();
  const name = props.initial.name;
  const [pad, setPad] = createSignal(props.initial);
  const [shown, setShown] = createSignal(props.initial.text);
  const [editing, setEditing] = createSignal(false);
  const actionFailure = createFailure();
  const refreshFailure = createFailure();
  let newestRefresh = 0;
  let missedWhileEditing = false;
  let closed = false;

  /** Server state always lands in `pad`; the text field only follows it while the user is not typing, or their keystrokes would vanish. */
  const adopt = (next: Pad) => {
    setPad(next);

    if (editing()) missedWhileEditing = true;
    else setShown(next.text);
  };

  const refresh = () =>
    refreshFailure.run(async () => {
      newestRefresh += 1;

      const mine = newestRefresh;
      const current = (await connected.app.rpc("pad.list", null)).find((listed) => listed.name === name);

      if (current && mine === newestRefresh) adopt(current);
    });

  onCleanup(
    connected.events.subscribe((event) => {
      if (event.name === "pad.changed" && event.data.name === name) void refresh();
    }),
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
      if (text !== pad().text) adopt(await connected.app.rpc("pad.write", { name, text }));
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
    await write(typed);

    if (missedWhileEditing) {
      missedWhileEditing = false;
      await refresh();
    }
  };

  return (
    <>
      <p>
        {ownerMark(pad().owner)} {name} <span class="light">owned by {connected.rail.nameOf(pad().owner)}</span>{" "}
        <button
          type="button"
          class="light"
          style={{ all: "unset", cursor: "pointer", color: "var(--light)" }}
          onClick={() => void exportToFile()}
        >
          export .md
        </button>
      </p>
      <Show when={actionFailure.message()}>{(message) => <ErrorLine message={message()} />}</Show>
      <Show when={refreshFailure.message()}>{(message) => <ErrorLine message={message()} />}</Show>
      <textarea
        aria-label="text"
        style={{ "font-family": "var(--mono)", width: "100%", "min-height": "16em" }}
        readOnly={!ownedByUser()}
        value={shown()}
        onFocus={() => setEditing(ownedByUser())}
        onBlur={(blurred) => void leaveField(blurred.currentTarget.value)}
      />
      <Show when={!ownedByUser()}>
        <input
          aria-label="append"
          placeholder="append"
          onKeyDown={(key) => {
            const field = key.currentTarget;

            if (key.key !== "Enter" || field.value === "") return;

            void append(field.value).then(() => {
              if (actionFailure.message() === null) field.value = "";
            });
          }}
        />
      </Show>
    </>
  );
};

/** Body of a Pad's Drawer: the Pad is read once, as the user's Touch, after the history that last-touch line shows. */
export const PadDrawer = (props: { name: string; onLateFailure: (message: string) => void }) => {
  const connected = useConnectedProject();

  return (
    <ItemDrawer item={`pad:${props.name}`} read={() => connected.app.rpc("pad.read", { name: props.name })}>
      {(pad) => <PadBody initial={pad} onLateFailure={props.onLateFailure} />}
    </ItemDrawer>
  );
};
