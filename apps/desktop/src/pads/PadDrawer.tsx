import { createSignal, onCleanup, Show } from "solid-js";
import type { Pad } from "@contracts/pad/Pad";
import { ItemDrawer } from "../drawer/ItemDrawer";
import { ErrorLine } from "../ink/ErrorLine";
import { useWorkspace } from "../state/workspace";
import { createFailure } from "./failure";
import { ownerMark } from "./owner";

const PadBody = (props: { initial: Pad }) => {
  const workspace = useWorkspace();
  const [pad, setPad] = createSignal(props.initial);
  const [editing, setEditing] = createSignal(false);
  const failure = createFailure();
  const name = props.initial.name;

  const refresh = () =>
    failure.run(async () => {
      const current = (await workspace.app.rpc("pad.list", null)).find((listed) => listed.name === name);

      if (current && !editing()) setPad(current);
    });

  onCleanup(
    workspace.events.subscribe((event) => {
      if (event.name === "pad.changed" && event.data.name === name && !editing()) void refresh();
    }),
  );

  const ownedByUser = () => pad().owner.kind === "user";

  const write = (text: string) =>
    failure.run(async () => {
      if (text !== pad().text) setPad(await workspace.app.rpc("pad.write", { name, text }));
    });

  const append = (text: string) =>
    failure.run(async () => {
      setPad(await workspace.app.rpc("pad.append", { name, text }));
    });

  const exportToFile = () =>
    failure.run(async () => {
      const path = await workspace.app.chooseSavePath(`${name}.md`);

      if (path !== null) await workspace.app.rpc("pad.export", { name, path });
    });

  return (
    <>
      <p>
        {ownerMark(pad().owner)} {name} <span class="light">owned by {workspace.rail.nameOf(pad().owner)}</span>{" "}
        <button type="button" class="word" onClick={() => void exportToFile()}>
          export .md
        </button>
      </p>
      <Show when={failure.message()}>{(message) => <ErrorLine message={message()} />}</Show>
      <textarea
        aria-label="text"
        style={{ "font-family": "var(--mono)", width: "100%", "min-height": "16em" }}
        readOnly={!ownedByUser()}
        value={pad().text}
        onFocus={() => setEditing(true)}
        onBlur={(blurred) => {
          setEditing(false);

          if (ownedByUser()) void write(blurred.currentTarget.value);
        }}
      />
      <Show when={!ownedByUser()}>
        <input
          aria-label="append"
          placeholder="append"
          onKeyDown={(key) => {
            if (key.key !== "Enter") return;

            const field = key.currentTarget;
            void append(field.value).then(() => {
              if (failure.message() === null) field.value = "";
            });
          }}
        />
      </Show>
    </>
  );
};

/** Body of a Pad's Drawer: the Pad is read once, as the user's Touch, after the history that last-touch line shows. */
export const PadDrawer = (props: { name: string }) => {
  const workspace = useWorkspace();

  return (
    <ItemDrawer item={`pad:${props.name}`} read={() => workspace.app.rpc("pad.read", { name: props.name })}>
      {(pad) => <PadBody initial={pad} />}
    </ItemDrawer>
  );
};
