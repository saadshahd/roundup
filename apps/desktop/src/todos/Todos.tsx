import { Icon } from "../ink/Icon";
import { createEffect, createMemo, on, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import type { Todo } from "@contracts/todo/Todo";
import { ErrorLine } from "../ink/ErrorLine";
import { KindGlyph } from "../ink/KindGlyph";
import { useConnectedProject } from "../state/connectedProject";
import { failureOf } from "./failureOf";
import { HomeMove } from "./HomeMove";
import { homeNameOf } from "./homes";
import { RowButton } from "./RowButton";
import { isRoom } from "../rail/layout";
import { isTodoOnRail } from "../rail/pads/owned";
import { useTodoList } from "./todoList";
import { TodoDrawer } from "./TodoDrawer";
import { ScopePair } from "./ScopePair";
import { doneTodos, kindOf, openBlockersOf, openTodos } from "./todoView";

/** U138: the row to focus once a completed Todo leaves the open list — the next one, else the previous, else none (the `+` button). */
export const rowAfter = (id: number, openIds: readonly number[]): number | null => {
  const at = openIds.indexOf(id);

  return openIds[at + 1] ?? openIds[at - 1] ?? null;
};

/** One open Todo as U15 draws it, with U35's `complete` and `waits on`; `nested` is the Rail's copy (U60), which leaves `data-id` to the Agent rows. */
export const OpenRow = (props: {
  nested?: boolean;
  todo: Todo;
  known: ReadonlyMap<number, Todo>;
  onOpen: (todo: Todo) => void;
  onComplete: (todo: Todo) => Promise<string | null>;
  registerRow?: (id: number, row: HTMLButtonElement | null) => void;
}) => {
  const connected = useConnectedProject();
  const waitingOn = createMemo(() => openBlockersOf(props.todo, props.known));
  const [hovered, setHovered] = createSignal(false);
  const [focused, setFocused] = createSignal(false);
  const [listOpen, setListOpen] = createSignal(false);
  const [failure, setFailure] = createSignal<string | null>(null);

  const complete = async () => setFailure(await props.onComplete(props.todo));

  // Matches U9's rule: a failure clears on the next click anywhere, not only inside its own row.
  onMount(() => {
    const clear = () => setFailure(null);

    document.addEventListener("click", clear);
    onCleanup(() => document.removeEventListener("click", clear));
  });

  onCleanup(() => props.registerRow?.(props.todo.id, null));

  return (
    <div
      data-id={props.nested ? undefined : props.todo.id}
      data-todo={props.todo.id}
      data-shelf-row={props.nested ? undefined : ""}
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
        <RowButton onClick={() => props.onOpen(props.todo)} ref={(row) => props.registerRow?.(props.todo.id, row)}>
          <KindGlyph kind={kindOf(props.todo)} /> #{props.todo.id} {props.todo.title}
        </RowButton>
        <Show when={hovered() || focused() || listOpen()}>
          <HomeMove
            id={props.todo.id}
            home={props.todo.home}
            popover
            onOpenChange={setListOpen}
            choose={async (home) => {
              const message = await failureOf(() => connected.app.rpc("todo.move", { id: props.todo.id, home }));

              setFailure(message);

              return message;
            }}
          />
          <button type="button" class="word complete" onClick={() => void complete()}>
            complete
          </button>
        </Show>
      </div>
      <Show
        when={failure()}
        fallback={
          <Show when={waitingOn().length > 0}>
            <p class="light" data-todo-waits style={{ "padding-left": "2ch" }}>
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
      <p class="light" data-todo-home>
        in {homeNameOf(connected.rail.nodes, props.todo.home)}
      </p>
    </div>
  );
};

/** The Shelf's `todos` list: open Todos in id order, done ones folded, and the inline field that makes a new one. */
export const Todos = () => {
  const connected = useConnectedProject();
  const todos = useTodoList();
  const [typing, setTyping] = createSignal(false);
  const [createFailure, setCreateFailure] = createSignal<string | null>(null);
  const [unfolded, setUnfolded] = createSignal(false);
  /** U60: an open Todo whose creator is an Agent on the Rail shows under that Agent instead. */
  const onShelf = createMemo(() => openTodos(todos.all()).filter((todo) => !isTodoOnRail(todo, connected.rail.nodes)));
  /** U159: the choice belongs to the Room it was made on, and selecting another row clears it, so returning reads `all`. */
  const [choice, setChoice] = createSignal<{ room: string; thisRoom: boolean } | null>(null);

  const selectedRoom = createMemo(() => {
    const id = connected.rail.selected();

    return connected.rail.nodes.some((node) => node.id === id && isRoom(node)) ? id : null;
  });

  createEffect(on(selectedRoom, () => setChoice(null), { defer: true }));

  const thisRoom = createMemo(() => selectedRoom() !== null && choice()?.room === selectedRoom() && choice()?.thisRoom === true);
  const open = createMemo(() => (thisRoom() ? onShelf().filter((todo) => todo.home === selectedRoom()) : onShelf()));
  const done = createMemo(() => doneTodos(todos.all()));
  const isEmpty = createMemo(() => todos.isLoaded() && todos.all().length === 0 && todos.failure() === null);
  const noneInRoom = createMemo(() => thisRoom() && todos.isLoaded() && open().length === 0 && todos.failure() === null);

  const rows = new Map<number, HTMLButtonElement>();
  let plusButton: HTMLButtonElement | undefined;

  // U138: a completed Todo leaves the open list only once its `todo.updated` Event has refetched it, so the row to focus
  // is held here until this list no longer carries `id`, then it is given to that target, else to the `+` button — but
  // only while focus is still unclaimed (null or the page body), so a request that outlives the user moving focus
  // elsewhere (typing in U16's field, Tab into the pane) never yanks it back.
  const [awaitingFocus, setAwaitingFocus] = createSignal<{ id: number; target: number | null }[]>([]);

  createEffect(() => {
    const openIds = new Set(open().map((todo) => todo.id));
    const waiting = awaitingFocus();

    if (waiting.every((request) => openIds.has(request.id))) return;

    setAwaitingFocus(
      waiting.filter((request) => {
        if (openIds.has(request.id)) return true;

        if (document.activeElement === null || document.activeElement === document.body) {
          const target = request.target !== null ? rows.get(request.target) : undefined;

          (target ?? plusButton)?.focus();
        }

        return false;
      }),
    );
  });

  // The field closes before the call so a second Enter cannot create the Todo twice.
  const create = async (title: string) => {
    setTyping(false);
    setCreateFailure(null);

    if (title === "") return;

    setCreateFailure(await failureOf(() => connected.app.rpc("todo.create", { title, body: null, blockers: null })));
  };

  const show = (todo: Todo) => connected.drawer.open(() => <TodoDrawer id={todo.id} todos={todos} />);

  const complete = async (todo: Todo) => {
    const target = rowAfter(todo.id, open().map((candidate) => candidate.id));
    const message = await failureOf(() => connected.app.rpc("todo.complete", { id: todo.id }));

    if (message === null) setAwaitingFocus((waiting) => [...waiting, { id: todo.id, target }]);

    return message;
  };

  return (
    <section aria-label="todos">
      <p>
        todos{" "}
        <button
          type="button"
          aria-label="add todo"
          class="word"
          ref={(button) => (plusButton = button)}
          onClick={() => {
            setCreateFailure(null);
            setTyping(true);
          }}
        >
          <Icon name="plus" />
        </button>
        <Show when={selectedRoom()}>
          {(room) => <ScopePair thisRoom={thisRoom()} onChange={(value) => setChoice({ room: room(), thisRoom: value })} />}
        </Show>
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
      <Show when={noneInRoom()}>
        <p>no todos in this room</p>
      </Show>
      <For each={open()}>
        {(todo) => (
          <OpenRow
            todo={todo}
            known={todos.byId()}
            onOpen={show}
            onComplete={complete}
            registerRow={(id, row) => {
              if (row) rows.set(id, row);
              else rows.delete(id);
            }}
          />
        )}
      </For>
      <Show when={done().length > 0}>
        <RowButton onClick={() => setUnfolded(!unfolded())}>
          <KindGlyph kind="done" decorative /> {done().length} done
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
