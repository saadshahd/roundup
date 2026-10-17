import { copy } from "../copy";
import { createContext, createEffect, createMemo, createSignal, For, on, onCleanup, Show, useContext } from "solid-js";
import type { Accessor, JSX } from "solid-js";
import { Portal } from "solid-js/web";
import { ErrorLine } from "../ink/ErrorLine";
import { registerKeys } from "../keys/registry";
import { AlertDialog } from "../ui/Dialog";

export const ShelfActions = createContext<{ mount: Accessor<HTMLDivElement | undefined>; open: Accessor<boolean> }>();

type Target<Id> = { id: Id; label: string; owner?: string | undefined };

type Question<Id> = { kind: "question" | "pending"; targets: readonly Target<Id>[]; done: boolean };

/** U459–U460: one frozen set, one question, and a retained remainder after readback. */
export const createBulkDelete = <Id,>(props: {
  selected: Accessor<readonly Id[]>;
  targets: Accessor<readonly Target<Id>[]>;
  done?: Accessor<readonly Target<Id>[]>;
  context: Accessor<string>;
  noun: "Todos" | "Pads";
  replace: (ids: readonly Id[]) => void;
  clear: () => void;
  cancelSelection: () => void;
  remove: (id: Id) => Promise<string | null>;
  reload: () => Promise<string | null>;
  reveal?: () => void;
}) => {
  const host = useContext(ShelfActions);
  const [question, setQuestion] = createSignal<Question<Id> | null>(null);
  const [failures, setFailures] = createSignal<readonly { target: Target<Id>; message: string }[]>([]);
  const [readFailure, setReadFailure] = createSignal<string | null>(null);
  let readback: (() => Promise<void>) | undefined;
  let revision = 0;
  let deleteButton: HTMLButtonElement | undefined;
  let keep: HTMLButtonElement | undefined;
  let barElement: HTMLDivElement | undefined;
  let returnTo: HTMLElement | null = null;
  let panel: HTMLDivElement | undefined;
  registerKeys({ kind: "layer", element: () => panel, active: () => question() !== null, modal: true }, []);

  const reset = () => {
    revision += 1;
    setQuestion(null);
    setFailures([]);
    setReadFailure(null);
    props.clear();
  };

  createEffect(on(() => `${props.context()}:${host?.open()}`, reset, { defer: true }));
  onCleanup(() => { revision += 1; });
  createEffect(() => {
    const held = question();

    if (held?.kind !== "question") return;
    const eligible = held.done ? props.done?.() ?? [] : props.targets();

    if (held.targets.some((target) => !eligible.some((row) => row.id === target.id))) setQuestion(null);
  });

  const ask = (done = false) => {
    if (question()) return;
    const targets = done ? props.done?.() ?? [] : props.targets().filter((target) => props.selected().includes(target.id));

    if (targets.length === 0) return;
    const unique = [...new Map(targets.map((target) => [target.id, { ...target }])).values()];
    returnTo = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setQuestion({ kind: "question", targets: unique, done });
  };

  /** The opener while it lives, else a Shelf control that does: a deleted row's button is gone. */
  const focusAfter = (): HTMLElement | null => {
    if (returnTo?.isConnected) return returnTo;

    if (deleteButton?.isConnected) return deleteButton;

    return host?.mount()?.closest<HTMLElement>(".shelf")?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]') ?? null;
  };

  const cancel = () => {
    const held = question();

    if (held?.kind !== "question") return;
    setQuestion(null);
  };

  const confirm = async () => {
    const held = question();

    if (held?.kind !== "question") return;
    const mine = revision;
    const eligible = held.done ? props.done?.() ?? [] : props.targets();
    const targets = held.targets.map((target) => ({ ...(eligible.find((row) => row.id === target.id) ?? target) }));
    setQuestion({ ...held, kind: "pending", targets });
    setFailures([]);
    const failed: { target: Target<Id>; message: string }[] = [];

    for (const target of targets) {
      const message = await props.remove(target.id);

      if (message !== null) failed.push({ target, message });
    }

    // A lost response can follow a committed delete; readback precedes retry.
    readback = async () => {
      const message = await props.reload();

      if (mine !== revision) return;
      setReadFailure(message);

      if (message !== null) return;

      if (held.done && failed.length > 0) props.reveal?.();
      const remaining = failed.filter(({ target }) => props.targets().some((row) => row.id === target.id));
      props.replace(remaining.map(({ target }) => target.id));
      setFailures(remaining);
      setQuestion(null);
      readback = undefined;
    };

    await readback();
  };

  const key = (event: KeyboardEvent) => {
    if (event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return;

    if (event.key === "Escape" && (question() || props.selected().length > 0)) {
      event.preventDefault();
      event.stopPropagation();

      if (question()?.kind === "question") cancel();
      else if (!question()) { props.cancelSelection(); setFailures([]); }
    } else if (event.key === "Backspace" && !event.repeat && props.selected().length > 0) {
      event.preventDefault();
      event.stopPropagation();
      ask();
    }
  };

  registerKeys({ kind: "region", element: () => barElement }, [{ chords: ["Escape", "Backspace"], when: () => props.selected().length > 0, run: key }]);

  // Escape cancels the selection from anywhere in the Shelf frame; a naming field or a blocker link keeps its own.
  registerKeys({ kind: "region", element: () => host?.mount()?.closest<HTMLElement>(".shelf") ?? undefined }, [{
    chords: ["Escape"],
    when: (event) => props.selected().length > 0 && !(event.target instanceof Element && event.target.closest("input, textarea, select, [contenteditable], .todo-blocker")),
    run: key,
  }]);

  createEffect(() => {
    if (props.selected().length === 0 && question() === null) setFailures([]);
  });

  const owners = createMemo(() => {
    const held = question();

    const targets = held?.kind === "pending" ? held.targets : props.targets().filter((target) =>
      held ? held.targets.some((row) => row.id === target.id) : props.selected().includes(target.id));

    return [...new Set(targets.flatMap((target) => target.owner ? [target.owner] : []))];
  });

  const bar = () => (
    <Show when={props.selected().length > 0 || failures().length > 0}>
      <div class="shelf-bulk" ref={(element) => { barElement = element; }} role="group" aria-label={copy.shelfDeletion}>
        <Show when={props.selected().length > 0}>
          <div class="shelf-bulk-line">
            <span>{props.selected().length}{copy.selected}</span>
            <button data-button="quiet" type="button" ref={(button) => { deleteButton = button; }} onClick={() => ask()}>{copy.deleteWord}{props.selected().length}</button>
            <button data-button="quiet" type="button" onClick={() => { props.cancelSelection(); setFailures([]); }}>{copy.cancel}</button>
          </div>
        </Show>
        <Show when={owners().length > 0 && question() === null}>
          <p>{copy.ownedBy}{owners().join(copy.comma)}{copy.agentsMayStillBeWritingDeletingDoesNot}</p>
        </Show>
        <Show when={failures().length > 0}>
          <ErrorLine message={copy.couldNotDelete} />
          <For each={failures()}>{(failure) => <p>{failure.target.label}{copy.colon}{failure.message}</p>}</For>
        </Show>
      </div>
    </Show>
  );

  /** The one question: a modal over the Shelf that names every frozen target and starts on `Keep`. */
  const dialog = () => (
    <AlertDialog.Root open={question() !== null} initialFocusEl={() => keep ?? null} finalFocusEl={focusAfter}
      onOpenChange={(details) => { if (!details.open) cancel(); }}>
      <Portal>
        <AlertDialog.Positioner>
          <AlertDialog.Content class="shelf-question" ref={(element) => { panel = element; }} onKeyDown={(press) => press.stopPropagation()}>
            <Show when={question()}>{(held) => <>
              <AlertDialog.Title>{copy.deleteWord}{held().targets.length}{held().done ? copy.doneCount : ""} {props.noun}{copy.questionMark}</AlertDialog.Title>
              <AlertDialog.Description>
                <ul class="shelf-question-targets">
                  <For each={held().targets}>{(target) => <li>{target.label}</li>}</For>
                </ul>
                <Show when={owners().length > 0}>
                  <p>{copy.ownedBy}{owners().join(copy.comma)}{copy.agentsMayStillBeWritingDeletingDoesNot}</p>
                </Show>
              </AlertDialog.Description>
              <Show when={readFailure()}>{(message) => <ErrorLine message={message()} />}</Show>
              <p class="shelf-question-words">
                <button data-button="quiet" type="button" class="word" disabled={held().kind === "pending"} ref={(button) => { keep = button; }} onClick={cancel}>{copy.keep}</button>
                <Show when={readFailure()} fallback={
                  <button data-button="quiet" type="button" class="word" disabled={held().kind === "pending"} onClick={() => void confirm()}>{copy.deleteWord}{held().targets.length}</button>
                }>
                  <button data-button="quiet" type="button" class="word" onClick={() => void readback?.()}>{copy.refresh}</button>
                </Show>
              </p>
            </>}</Show>
          </AlertDialog.Content>
        </AlertDialog.Positioner>
      </Portal>
    </AlertDialog.Root>
  );

  const content = () => <>{bar()}{dialog()}</>;

  const Bar = (): JSX.Element => host ? <Show when={host.mount()}>{(mount) => <Portal mount={mount()}>{content()}</Portal>}</Show> : content();

  return { ask, key, Bar, pending: () => question()?.kind === "pending" };
};
