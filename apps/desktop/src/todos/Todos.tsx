import { createMemo, createSignal, For, Show } from "solid-js";
import type { Todo } from "@contracts/todo/Todo";
import { ErrorLine } from "../ink/ErrorLine";
import { KindGlyph } from "../ink/KindGlyph";
import { useConnectedProject } from "../state/connectedProject";
import { failureOf } from "./failureOf";
import { RowButton } from "./RowButton";
import { createTodosState } from "./state";
import { TodoDrawer } from "./TodoDrawer";
import { doneTodos, kindOf, openBlockersOf, openTodos } from "./todoView";

type CreateField = { kind: "closed" } | { kind: "typing" } | { kind: "failed"; message: string };

/** The Shelf's `todos` list: open Todos in id order, done ones folded, and the inline field that makes a new one. */
export const Todos = () => {
  const workspace = useConnectedProject();
  const todos = createTodosState(workspace.app, workspace.events);
  const [field, setField] = createSignal<CreateField>({ kind: "closed" });
  const [unfolded, setUnfolded] = createSignal(false);
  const open = createMemo(() => openTodos(todos.all()));
  const done = createMemo(() => doneTodos(todos.all()));

  // The field closes before the call so a second Enter cannot create the Todo twice.
  const create = async (title: string) => {
    setField({ kind: "closed" });

    if (title === "") return;

    const message = await failureOf(() => workspace.app.rpc("todo.create", { title, body: null, blockers: null }));

    if (message !== null) setField({ kind: "failed", message });
  };

  const show = (todo: Todo) => workspace.drawer.open(() => <TodoDrawer id={todo.id} todos={todos} />);

  return (
    <section aria-label="todos">
      <p>
        todos{" "}
        <button type="button" class="word" onClick={() => setField({ kind: "typing" })}>
          + todo
        </button>
      </p>
      <Show when={todos.failure()}>{(message) => <ErrorLine message={message()} />}</Show>
      <Show when={field()} keyed>
        {(current) =>
          current.kind === "typing" ? (
            <input
              aria-label="new todo title"
              ref={(input) => queueMicrotask(() => input.focus())}
              onKeyDown={(key) => {
                if (key.key === "Enter") void create(key.currentTarget.value.trim());
                else if (key.key === "Escape") setField({ kind: "closed" });
              }}
            />
          ) : current.kind === "failed" ? (
            <ErrorLine message={current.message} />
          ) : null
        }
      </Show>
      <For each={open()}>
        {(todo) => (
          <div>
            <RowButton onClick={() => show(todo)}>
              <KindGlyph kind={kindOf(todo)} /> #{todo.id} {todo.title}
            </RowButton>
            <Show when={openBlockersOf(todo, todos.all()).length > 0}>
              <p class="light" style={{ "padding-left": "2ch" }}>
                waits on {openBlockersOf(todo, todos.all()).map((blocker) => `#${blocker.id}`).join(", ")}
              </p>
            </Show>
          </div>
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
