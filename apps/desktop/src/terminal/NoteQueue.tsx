import { captureFocus } from "../keys/focus";
import { For, Show, createEffect, createMemo } from "solid-js";
import type { RailNode } from "@contracts/agent/RailNode";
import type { Message } from "@contracts/message/Message";
import { copy } from "../copy";
import { ReservedLine } from "../ink/ReservedLine";
import { useConnectedProject } from "../state/connectedProject";
import { IconButton } from "../ui/IconButton";
import "./noteQueue.css";

/** The first line of a note, with `…` when more follows. */
const firstLine = (body: string): string => {
  const [first = "", ...more] = body.split("\n");

  return more.length > 0 ? `${first}${copy.ellipsis}` : first;
};

/** U3780, U3781, U3782: the Daemon's pending Messages under an Agent's Terminal; the user removes only a Message they wrote. */
export const NoteQueue = (props: { node: RailNode; focusTerminal: () => void }) => {
  const { notes, rail, daemonExit } = useConnectedProject();
  const agent = () => props.node.id;
  const rows = createMemo(() => notes.rows(agent()));
  const ids = createMemo(() => rows().map((message) => message.id));
  let root: HTMLElement | undefined;
  let afterRemoval: { agent: string; removed: number; next: number | null; focus: ReturnType<typeof captureFocus> } | null = null;

  createEffect(() => notes.load(agent()));

  const senderOf = (message: Message): string | null => {
    if (message.from.kind === "user") return null;

    return rail.nodes.find((node) => node.id === message.from.id)?.name ?? copy.decisionMaker;
  };

  // A removed row hands focus to the next row's ×, or to the Terminal when none is left.
  createEffect(() => {
    const present = ids();
    const pending = afterRemoval;

    if (pending === null || pending.agent !== agent() || present.includes(pending.removed)) return;

    afterRemoval = null;

    pending.focus(() => {
      if (pending.next === null) props.focusTerminal();
      else root?.querySelector<HTMLElement>(`[data-note="${pending.next}"] [data-remove]`)?.focus();
    });
  });

  const remove = (message: Message): void => {
    const at = ids().indexOf(message.id);

    afterRemoval = { agent: agent(), removed: message.id, next: ids()[at + 1] ?? null, focus: captureFocus() };
    void notes.remove(message);
  };

  const failure = () => notes.failure(agent());

  // P3557c: dismissing clears the shown line only; Messages, the Agent's draft and recipients stay as they are.
  const dismiss = (): void => {
    const shown = notes.selected(agent());

    if (shown !== null) notes.dismiss(shown.key);
  };

  return (
    <Show when={rows().length > 0 || failure()}>
      <section ref={(element) => { root = element; }} class="note-queue" aria-label={copy.noteTo(props.node.name)}>
        <Show when={rows().length > 1}>
          <header class="note-queue-head"><span class="note-queue-count">{copy.notesWaiting(rows().length)}</span></header>
        </Show>
        <ul class="note-queue-rows">
          <For each={rows()}>{(message) => (
            <li class="note-queue-row" data-note={message.id} aria-description={message.body}>
              <Show when={senderOf(message)}>{(name) => <span class="note-queue-from">{name()}</span>}</Show>
              <span class="note-queue-text">{firstLine(message.body)}</span>
              <Show when={message.from.kind === "user"}>
                <IconButton data-remove aria-label={copy.removeQueuedNote(firstLine(message.body))} disabled={daemonExit() !== null} onClick={() => remove(message)}>{copy.removeGlyph}</IconButton>
              </Show>
            </li>
          )}</For>
        </ul>
        <ReservedLine message={failure()}>
          <Show when={failure()}><button data-button="quiet" type="button" class="word" onClick={dismiss}>{copy.dismissFailure}</button></Show>
        </ReservedLine>
      </section>
    </Show>
  );
};
