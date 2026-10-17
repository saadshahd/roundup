import { AlertDialog } from "../ui/Dialog";
import { registerKeys } from "../keys/registry";
import { IconButton } from "../ui/IconButton";
import { copy } from "../copy";
import { Icon } from "../ink/Icon";
import { createEffect, createSignal, For, on, Show } from "solid-js";
import type { Todo } from "@contracts/todo/Todo";
import { ErrorLine } from "../ink/ErrorLine";
import { KindGlyph } from "../ink/KindGlyph";
import { TickBox } from "./TickBox";
import { TodoHistory } from "./TodoHistory";
import "./drawer.styles.css";
import { useConnectedProject } from "../state/connectedProject";
import { EditableText } from "./EditableText";
import { failureOf } from "./failureOf";
import { RowButton } from "./RowButton";
import type { TodosState } from "./state";
import { blockedBy, blockersOf, kindOf, offeredAsBlockers } from "./todoView";

const TodoLine = (props: { todo: Todo }) => (
  <p style={{ "white-space": "pre-wrap" }}>
    <KindGlyph kind={kindOf(props.todo)} /> <span class="mono">#{props.todo.id}</span>{`  ${props.todo.title}`}
  </p>
);

/** The current Todo follows list Events; TodoHistory captures and acknowledges this opening's read. */
export const TodoDrawer = (props: { id: number; todos: TodosState }) => {
  const { app, drawer } = useConnectedProject();

  const [failure, setFailure] = createSignal<string | null>(null);
  const [offering, setOffering] = createSignal(false);
  const [removedByMe, setRemovedByMe] = createSignal(false);
  const [asking, setAsking] = createSignal(false);
  const [deleting, setDeleting] = createSignal(false);
  let deleteButton: HTMLButtonElement | undefined;

  const attempt = async <Result,>(call: () => Promise<Result>) => {
    const message = await failureOf(call);

    setFailure(message);

    return message;
  };

  const [completed, setCompleted] = createSignal(false);
  const todo = () => props.todos.byId().get(props.id);
  let titleRow: HTMLDivElement | undefined;

  const complete = () => attempt(async () => {
    const focused = document.activeElement === titleRow?.querySelector("input");
    await app.rpc("todo.complete", { id: props.id });
    setCompleted(true);

    const stayed = document.activeElement === titleRow?.querySelector("input") || document.activeElement === document.body;

    if (focused && stayed) titleRow?.querySelector<HTMLElement>("p[tabindex]")?.focus();
  });

  // Picks run one after another, each from the list the previous one answered: the list state only catches up when the Event arrives.
  const [answered, setAnswered] = createSignal<Todo | null>(null);
  let picks: Promise<unknown> = Promise.resolve();

  // The list state does not refire when only this Todo's fields change, so the cache watches the blockers itself.
  createEffect(on(() => todo()?.blockers.join(","), () => setAnswered(null), { defer: true }));

  // U152, U138: a keyboard step that removes the focused button hands focus to `+ blocker`, never the page body.
  let blockerButton: HTMLButtonElement | undefined;
  let removing = false;

  createEffect(
    on(
      () => todo()?.blockers.join(","),
      () => {
        if (!removing) return;

        removing = false;
        queueMicrotask(() => {
          if (!document.activeElement || document.activeElement === document.body) blockerButton?.focus();
        });
      },
      { defer: true },
    ),
  );

  const blockerBase = () => answered() ?? todo();

  const changeBlockers = (next: (blockers: number[]) => number[]) => {
    const pick = () =>
      attempt(async () => {
        const base = blockerBase();

        if (!base) return;

        setAnswered(await app.rpc("todo.setBlockers", { id: props.id, blockers: next(base.blockers) }));
      });

    // A bug that rejects one pick is reported, and the picks behind it still run.
    picks = picks.then(pick, (bug) => {
      reportError(bug);

      return pick();
    });
  };

  const addBlocker = (blocker: number) => changeBlockers((blockers) => [...blockers, blocker]);

  /** U152: the other blockers stay, in order. */
  const removeBlocker = (blocker: number) => {
    removing = true;
    changeBlockers((blockers) => blockers.filter((id) => id !== blocker));
  };

  const keep = () => {
    setAsking(false);
  };

  // The confirmation is a layer nested in the Drawer's panel, so Escape reaches it first.
  let confirmation: HTMLDivElement | undefined;
  let keepButton: HTMLButtonElement | undefined;

  registerKeys({ kind: "layer", element: () => confirmation, active: () => asking() && !!todo(), modal: true }, []);

  return (
    <>
      <div class="todo-detail">
        <AlertDialog.Root open={asking() && !!todo()} initialFocusEl={() => keepButton ?? null} finalFocusEl={() => deleteButton ?? null} onOpenChange={({ open }) => { if (!open && !deleting()) keep(); }}>
          <AlertDialog.Positioner>
          <AlertDialog.Content class="todo-delete-confirm" ref={(element) => { confirmation = element; }}>
            <AlertDialog.Title>{copy.deleteTodoQuestion}</AlertDialog.Title>
            <Show when={failure()}>{(message) => <ErrorLine message={message()} />}</Show>
            <button data-button="quiet"
              type="button"
              class="word light"
              disabled={deleting()}
              onClick={() => {
                if (deleting()) return;
                setDeleting(true);
                void attempt(async () => {
                  await app.rpc("todo.delete", { id: props.id });
                  setRemovedByMe(true);
                  drawer.close();
                }).then(() => setDeleting(false));
              }}
            >{copy.delete}</button>{" "}
            <button data-button="quiet"
              type="button"
              class="word light"
              ref={(button) => { keepButton = button; }}
              onClick={keep}
            >{copy.keep}</button>
          </AlertDialog.Content>
          </AlertDialog.Positioner>
        </AlertDialog.Root>
        <Show when={!asking() && failure()}>{(message) => <ErrorLine message={message()} />}</Show>
          <Show
            when={todo()}
            fallback={
              <Show when={!removedByMe() && !deleting()}>
                <ErrorLine message={copy.todoDeleted(props.id)} />
              </Show>
            }
          >
            {(current) => (
              <>
                <div class="todo-detail-title" ref={(element) => { titleRow = element; }}>
                  <TickBox todo={{ ...current(), done: completed() || current().done }} onComplete={complete} />
                  <EditableText
                    name="title"
                    multiline={false}
                    value={current().title}
                    commit={(title) => attempt(() => app.rpc("todo.update", { id: props.id, title, body: null }))}
                  />
                </div>
                <div class="todo-detail-body">
                <EditableText
                  name="body"
                  multiline
                  value={current().body}
                  commit={(body) => attempt(() => app.rpc("todo.update", { id: props.id, title: null, body }))}
                />
                </div>
                <Show when={blockersOf(current(), props.todos.byId()).length > 0}>
                  <div class="todo-dependencies" style={{ display: "grid", "grid-template-columns": "1fr auto", "column-gap": "var(--space-2)" }}>
                    <p class="light" style={{ "grid-column": "1 / -1" }}>{copy.waitsOn}</p>
                    <For each={blockersOf(current(), props.todos.byId())}>
                      {(blocker) => (
                        <>
                          <TodoLine todo={blocker} />
                          <button data-button="quiet" type="button" class="word light" aria-label={copy.removeBlocker(blocker.id)} onClick={() => removeBlocker(blocker.id)}>{copy.remove}</button>
                        </>
                      )}
                    </For>
                  </div>
                </Show>
                <Show when={blockedBy(current(), props.todos.all()).length > 0}>
                  <div class="todo-dependencies">
                    <p class="light">{copy.blocks}</p>
                    <For each={blockedBy(current(), props.todos.all())}>{(blocked) => <TodoLine todo={blocked} />}</For>
                  </div>
                </Show>
                <p>
                  <button data-button="add" type="button" class="word" ref={(element) => { blockerButton = element; }} onClick={() => setOffering(!offering())}>
                    <Icon name="plus" />{copy.blockerWord}</button>
                </p>
                <Show when={offering()}>
                  <For each={offeredAsBlockers(answered() ?? current(), props.todos.all())}>
                    {(other) => (
                      <RowButton
                        onClick={() => {
                          setOffering(false);
                          blockerButton?.focus();
                          addBlocker(other.id);
                        }}
                      >
                        <KindGlyph kind={kindOf(other)} /> <span class="mono">#{other.id}</span>{`  ${other.title}`}
                      </RowButton>
                    )}
                  </For>
                </Show>
                <TodoHistory id={props.id} />
              </>
            )}
          </Show>
      </div>
      <Show when={todo()}>
        <IconButton class="todo-delete" aria-label={copy.deleteTodo}
          ref={(button) => { deleteButton = button; }}
          onClick={() => { setFailure(null); setAsking(true); }}>
          <Icon name="trash" />
        </IconButton>
      </Show>
    </>
  );
};
