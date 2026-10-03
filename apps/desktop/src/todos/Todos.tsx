import { createMemo, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import type { Todo } from "@contracts/todo/Todo";
import { ErrorLine } from "../ink/ErrorLine";
import { KindGlyph } from "../ink/KindGlyph";
import { useConnectedProject } from "../state/connectedProject";
import { failureOf } from "./failureOf";
import { RowButton } from "./RowButton";
import { createTodosState } from "./state";
import { TodoDrawer } from "./TodoDrawer";
import { doneTodos, kindOf, openBlockersOf, openTodos } from "./todoView";

const OpenRow = (props: {
  todo: Todo;
  known: ReadonlyMap<number, Todo>;
  onOpen: (todo: Todo) => void;
  onComplete: (todo: Todo) => Promise<string | null>;
}) => {
  const waitingOn = createMemo(() => openBlockersOf(props.todo, props.known));
  const [hovered, setHovered] = createSignal(false);
  const [focused, setFocused] = createSignal(false);
  const [failure, setFailure] = createSignal<string | null>(null);

  const complete = async () => setFailure(await props.onComplete(props.todo));

  // Matches U9's rule: a failure clears on the next click anywhere, not only inside its own row.
  onMount(() => {
    const clear = () => setFailure(null);

    document.addEventListener("click", clear);
    onCleanup(() => document.removeEventListener("click", clear));
  });

  return (
    <div
      data-id={props.todo.id}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onFocusIn={() => setFocused(true)}
      onFocusOut={(event) => {
        const next = event.relatedTarget;

        if (next instanceof Node && event.currentTarget.contains(next)) return;

        setFocused(false);
      }}
    >
      <div class="row-head">
        <RowButton onClick={() => props.onOpen(props.todo)}>
          <KindGlyph kind={kindOf(props.todo)} /> #{props.todo.id} {props.todo.title}
        </RowButton>
        <Show when={hovered() || focused()}>
          <button type="button" class="word complete" onClick={() => void complete()}>
            complete
          </button>
        </Show>
      </div>
      <Show
        when={failure()}
        fallback={
          <Show when={waitingOn().length > 0}>
            <p class="light" style={{ "padding-left": "2ch" }}>
              waits on <For each={waitingOn()}>{(blocker, index) => (
                <>
                  <Show when={index() > 0}>{", "}</Show>
                  <button type="button" class="word" onClick={() => props.onOpen(blocker)}>
                    #{blocker.id}
                  </button>
                </>
              )}</For>
            </p>
          </Show>
        }
      >
        {(message) => <ErrorLine message={message()} />}
      </Show>
    </div>
  );
};

/** The Shelf's `todos` list: open Todos in id order, done ones folded, and the inline field that makes a new one. */
export const Todos = () => {
  const connected = useConnectedProject();
  const todos = createTodosState(connected.app, connected.events);
  const [typing, setTyping] = createSignal(false);
  const [createFailure, setCreateFailure] = createSignal<string | null>(null);
  const [unfolded, setUnfolded] = createSignal(false);
  const open = createMemo(() => openTodos(todos.all()));
  const done = createMemo(() => doneTodos(todos.all()));
  const isEmpty = createMemo(() => todos.isLoaded() && todos.all().length === 0 && todos.failure() === null);

  // The field closes before the call so a second Enter cannot create the Todo twice.
  const create = async (title: string) => {
    setTyping(false);
    setCreateFailure(null);

    if (title === "") return;

    setCreateFailure(await failureOf(() => connected.app.rpc("todo.create", { title, body: null, blockers: null })));
  };

  const show = (todo: Todo) => connected.drawer.open(() => <TodoDrawer id={todo.id} todos={todos} />);

  const complete = (todo: Todo) => failureOf(() => connected.app.rpc("todo.complete", { id: todo.id }));

  return (
    <section aria-label="todos">
      <p>
        todos{" "}
        <button type="button" class="word" onClick={() => {
            setCreateFailure(null);
            setTyping(true);
          }}>
          +
        </button>
      </p>
      <Show when={todos.failure()}>{(message) => <ErrorLine message={message()} />}</Show>
      <Show when={createFailure()}>{(message) => <ErrorLine message={message()} />}</Show>
      <Show when={typing()}>
        <input
          aria-label="new todo title"
          ref={(input) => queueMicrotask(() => input.focus())}
          onKeyDown={(key) => {
            if (key.key === "Enter") void create(key.currentTarget.value.trim());
            else if (key.key === "Escape") {
              setTyping(false);
              setCreateFailure(null);
            }
          }}
        />
      </Show>
      <Show when={isEmpty()}>
        <p>no todos yet</p>
      </Show>
      <For each={open()}>
        {(todo) => (
          <OpenRow todo={todo} known={todos.byId()} onOpen={show} onComplete={complete} />
        )}
      </For>
      <Show when={done().length > 0}>
        <RowButton onClick={() => setUnfolded(!unfolded())}>
          <KindGlyph kind="done" /> {done().length} done
        </RowButton>
        <Show when={unfolded()}>
          <For each={done()}>
            {(todo) => (
              <RowButton onClick={() => show(todo)}>
                <KindGlyph kind={kindOf(todo)} /> #{todo.id} {todo.title}
              </RowButton>
            )}
          </For>
        </Show>
      </Show>
    </section>
  );
};
