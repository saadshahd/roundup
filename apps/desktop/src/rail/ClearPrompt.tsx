import { queueFocus } from "../keys/focus";
import { ReservedLine } from "../ink/ReservedLine";
import { copy } from "../copy";
import { onMount, Show } from "solid-js";
import type { RailNode } from "@contracts/agent/RailNode";
import { useConnectedProject } from "../state/connectedProject";
import { registerKeys } from "../keys/registry";
import { clearingOf } from "./clear";

/** U350: the Workstream's reason prompt, open once `Clear…` in the `⋯` menu is chosen. */
export const ClearPrompt = (props: { node: RailNode; onHide: () => void }) => {
  const { app, events, rail, drafts } = useConnectedProject();
  const clearing = clearingOf(app, events, props.node.id, drafts);
  let field: HTMLTextAreaElement | undefined;
  let form: HTMLFormElement | undefined;
  const hide = () => { clearing.keep(); props.onHide(); };

  registerKeys({ kind: "layer", element: () => form, active: () => clearing.phase() !== "closed", modal: false }, [{
    chords: ["Escape"],
    when: (press) => press.target === document || document.activeElement === document.body || (press.target instanceof Node && !!form?.contains(press.target)),
    run: hide,
  }]);

  const subagents = () => {
    const below = new Set([props.node.id]);

    for (let grown = true; grown; ) {
      grown = false;

      for (const node of rail.nodes) {
        if (node.parent !== null && below.has(node.parent) && !below.has(node.id)) {
          below.add(node.id);
          grown = true;
        }
      }
    }

    return rail.nodes.filter((node) => node.kind === "agent" && below.has(node.id)).length;
  };

  onMount(() => {
    const prompt = clearing;
    const selection = prompt.selection();

    if (prompt.phase() !== "asking") return;

    queueFocus(() => {
      if (!field?.isConnected) return;

      field.focus();

      if (selection) field.setSelectionRange(selection.start, selection.end, selection.direction);
    });
  });

  const pending = () => clearing.phase() === "pending";

  return (
    <Show when={clearing.phase() !== "closed"}>
      <form
        ref={(element) => { form = element; }}
        class="clear-prompt"
        data-clear-prompt
        onSubmit={(submit) => {
          submit.preventDefault();
          void clearing.submit(false);
        }}

      >
        <label class="clear-title">{copy.whyClear}<textarea
            ref={(element) => (field = element)}
            class="clear-reason"
            rows={1}
            wrap="off"
            value={clearing.reason()}
            disabled={pending()}
            onInput={(input) => clearing.setReason(input.currentTarget.value, { start: input.currentTarget.selectionStart, end: input.currentTarget.selectionEnd, direction: input.currentTarget.selectionDirection })}
            onSelect={(event) => { if (event.currentTarget.isConnected && !pending()) clearing.select({ start: event.currentTarget.selectionStart, end: event.currentTarget.selectionEnd, direction: event.currentTarget.selectionDirection }); }}
            onKeyDown={(press) => {
              if (press.key === "Enter" && !press.isComposing) {
                press.preventDefault();
                press.currentTarget.form?.requestSubmit();
              }
            }}
          />
        </label>
        <p class="clear-line light">
          {pending()
            ? copy.theDoorIsWritingTheHandoff
            : copy.theDoorWritesAHandoffKeepsWhatYou(subagents())}
        </p>
        <ReservedLine message={clearing.failure()?.message} />
        <p class="clear-words">
          <button type="submit" class="word" data-button="quiet" disabled={!clearing.canSubmit()}>{copy.clearWithHandoff}</button>{" "}
          <Show when={clearing.offersWithoutHandoff()}>
            <button type="button" class="word" data-button="quiet" disabled={!clearing.canSubmit()} onClick={() => void clearing.submit(true)}>{copy.clearWithoutHandoff}</button>{" "}
          </Show>
          <button type="button" class="word" data-button="quiet" disabled={pending()} onClick={hide}>{copy.keep}</button>
        </p>
      </form>
    </Show>
  );
};
