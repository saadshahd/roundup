import ChevronUp from "lucide-solid/icons/chevron-up";
import { Tooltip } from "../ui/Tooltip";
import { Portal } from "solid-js/web";
import { createNear } from "../ink/near";
import { IconButton } from "../ui/IconButton";
import { copy } from "../copy";
import { createBulkDelete } from "../shelf/BulkDelete";
import { isTodoOfRunning } from "../rail/pads/owned";
import { createSelection } from "../shelf/selection";
import { TickBox } from "./TickBox";
import { Icon } from "../ink/Icon";
import type { JSX } from "solid-js";
import { createEffect, createMemo, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import type { Todo } from "@contracts/todo/Todo";
import { ErrorLine } from "../ink/ErrorLine";
import { HomeMove } from "./HomeMove";
import { homeNameOf } from "./homes";
import { ShelfRow } from "../ink/ShelfRow";
import { KindGlyph } from "../ink/KindGlyph";
import { useConnectedProject } from "../state/connectedProject";
import { failureOf } from "./failureOf";
import { RowButton } from "./RowButton";
import { createSortedList } from "./sortedList";
import { Sortable } from "../ui/sortable";
import { useShelfScope } from "./shelfScope";
import { useTodoList } from "./todoList";
import { TodoDrawer } from "./TodoDrawer";
import { shownTodos } from "./shownTodos";
import { doneTodos, kindOf, openBlockersOf } from "./todoView";

const KIND_LABEL = { idle: copy.idle, blocked: copy.blocked, done: copy.done } as const;

/** U138: the row to focus once a completed Todo leaves the open list — the next one, else the previous, else none (the `+` button). */
export const rowAfter = (id: number, openIds: readonly number[]): number | null => {
  const at = openIds.indexOf(id);

  return openIds[at + 1] ?? openIds[at - 1] ?? null;
};

/** U490a: what a list ordering its rows through `ui/Sortable` hands one `OpenRow`. */
export type SortedRow = {
  /** The row's frame, which the Sortable moves. */
  frame: (el: HTMLElement | null) => void;
  /** The open button: the Sortable's handle, and the element that keeps focus after a move. */
  button: (el: HTMLButtonElement | null) => void;
  /** Where a failed move shows. */
  fail: (show: (message: string | null) => void) => void;
  /** `⌥↑` and `⌥↓`. */
  key: (event: KeyboardEvent) => void;
  step: (by: -1 | 1) => void;
};

/** One open Todo as U15 draws it, with U35's `complete` and `waits on`; `nested` is the Rail's copy (U60), which leaves `data-id` to the Agent rows. */
export const OpenRow = (props: {
  nested?: boolean;
  pressed?: boolean;
  selectionActive?: boolean;
  onPick?: (todo: Todo, event: MouseEvent) => void;
  onSelect?: (todo: Todo) => void;
  onSelectionKey?: (todo: Todo, event: KeyboardEvent) => void;
  /** U153: the row whose Drawer is open. */
  selected?: boolean;
  todo: Todo;
  known: ReadonlyMap<number, Todo>;
  onOpen: (todo: Todo) => void;
  onComplete: (todo: Todo) => Promise<string | null>;
  registerRow?: (id: number, row: HTMLButtonElement | null) => void;
  /** U490a: a row ordered through `ui/Sortable` instead. */
  sorted?: SortedRow;
}) => {
  const connected = useConnectedProject();
  const near = props.nested ? createNear() : null;
  const [listOpen, setListOpen] = createSignal(false);
  const waitingOn = createMemo(() => openBlockersOf(props.todo, props.known));
  const [failure, setFailure] = createSignal<string | null>(null);

  const complete = async () => {
    const message = await props.onComplete(props.todo);

    setFailure(message);

    return message;
  };

  // Matches U9's rule: a failure clears on the next click anywhere, not only inside its own row.
  onMount(() => {
    const clear = () => setFailure(null);

    document.addEventListener("click", clear);
    onCleanup(() => document.removeEventListener("click", clear));
  });

  onCleanup(() => {
    props.registerRow?.(props.todo.id, null);
    props.sorted?.frame(null);
    props.sorted?.button(null);
  });

  props.sorted?.fail(setFailure);

  const head = (
    <div class="row-head">
        <RowButton
          hang={false}
          label={copy.todoRow(KIND_LABEL[kindOf(props.todo)], props.todo.id, props.todo.title)}
          tooltip={failure() === null ? props.todo.title : undefined}
          pressed={props.pressed}
          selectionActive={props.selectionActive}
          onClick={(event) => props.onPick ? props.onPick(props.todo, event) : props.onOpen(props.todo)}
          reorderable={props.sorted !== undefined}
          onKeyDown={(event) => {
            props.onSelectionKey?.(props.todo, event);

            props.sorted?.key(event);
          }}
          ref={(row) => {
            props.registerRow?.(props.todo.id, row);
            props.sorted?.button(row);
          }}
        >
          <span class="shelf-title">{props.todo.title}</span>
        </RowButton>
        <Show when={!props.nested && (props.sorted || props.onSelect)}>
          <div class="todo-row-actions" role="group" aria-label={copy.rowActionsFor(props.todo.id)}>
            <Show when={props.onSelect}><IconButton aria-label={copy.selectTodo} aria-pressed={props.pressed === true} onClick={() => props.onSelect?.(props.todo)}><Icon name="check" /></IconButton></Show>
            <Show when={props.sorted}>
              <IconButton aria-label={copy.moveUp} onClick={() => props.sorted?.step(-1)}><ChevronUp class="icon" aria-hidden="true" width={16} height={16} stroke-width={1.75} /></IconButton>
              <IconButton aria-label={copy.moveDown} onClick={() => props.sorted?.step(1)}><Icon name="down" /></IconButton>
            </Show>
          </div>
        </Show>
        <Show when={props.nested}>
          <HomeMove
            id={props.todo.id}
            home={props.todo.home}
            onOpenChange={setListOpen}
            choose={async (home) => {
              const message = await failureOf(() => connected.app.rpc("todo.move", { id: props.todo.id, home }));

              setFailure(message);

              return message;
            }}
          />
          <button data-button="row-action" type="button" class="word complete" onClick={() => void complete()}>{copy.complete}</button>
        </Show>
      </div>
  );

  return (
    <ShelfRow
      glyph={props.nested ? <KindGlyph kind={kindOf(props.todo)} /> : <>
        <TickBox todo={props.todo} onComplete={complete} />
        <Show when={props.todo.blocked}><KindGlyph kind="blocked" /></Show>
      </>}
      data-id={props.nested ? undefined : props.todo.id}
      data-todo={props.todo.id}
      data-actions-open={near?.near() || listOpen() ? "true" : undefined}
      data-shelf-row={props.nested ? undefined : ""}
      data-selected={props.selected ? "true" : undefined}
      class={props.nested ? "todo-row" : "todo-row todo-inset"}
      head={props.nested ? undefined : head}
      {...near?.handlers}
      ref={(frame) => {
        props.sorted?.frame(frame);
      }}
    >
      <Show when={props.nested}>{head}</Show>
      <Show when={props.todo.body.trim() !== ""}>
        <p class="item-description" data-todo-description>{props.todo.body.trim()}</p>
      </Show>
      <Show when={failure()}>{(message) => <ErrorLine message={message()} />}</Show>
      <Show when={props.nested || (failure() === null && waitingOn().length > 0)}>
        <p class="shelf-meta" data-todo-meta>
          <Show when={props.nested}><span data-mono>{copy.todoNumberMark}{props.todo.id}</span>{" "}</Show>
          <Show when={failure() === null && waitingOn().length > 0}>
            <Tooltip.Root><Tooltip.Trigger asChild={(trigger) => <span {...trigger()} data-todo-waits>
              <span>{copy.waitsOnWord}</span><span class="todo-blockers"><For each={waitingOn()}>{(blocker, index) => (
                <>
                  <Show when={index() > 0}><span>{copy.comma}</span></Show>
                  <button data-button="quiet" type="button" class="word todo-blocker" data-mono={props.nested ? "" : undefined} aria-label={props.nested ? undefined : copy.openBlocker(blocker.title)} onClick={() => props.onOpen(blocker)}>
                    {props.nested ? `#${blocker.id}` : blocker.title}
                  </button>
                </>
              )}</For></span>
            </span>} />
              <Portal><Tooltip.Positioner><Tooltip.Content class="todo-tooltip">{copy.waitsOnTitles(waitingOn().map((blocker) => blocker.title).join(copy.comma))}</Tooltip.Content></Tooltip.Positioner></Portal>
            </Tooltip.Root>
          </Show>
          <Show when={props.nested}>
            {" "}<span data-todo-home>{copy.in}<span data-mono>{homeNameOf(connected.rail.nodes, props.todo.home)}</span></span>
          </Show>
        </p>
      </Show>
    </ShelfRow>
  );
};

/** The Shelf's `todos` list: open Todos in id order, done ones folded, and the inline field that makes a new one. */
export const Todos = () => {
  const connected = useConnectedProject();
  const todos = useTodoList();
  const [typing, setTyping] = createSignal(false);
  const [createFailure, setCreateFailure] = createSignal<string | null>(null);
  const [unfolded, setUnfolded] = createSignal(false);
  const scope = useShelfScope(connected);
  const selectedWorkstream = scope.workstream;
  const thisWorkstream = createMemo(() => selectedWorkstream() !== null && !scope.showAll());

  const open = shownTodos(connected, todos);

  const selection = createSelection(() => [...open(), ...(unfolded() ? done() : [])].map((todo) => todo.id), () => `${connected.rail.selected()}:${scope.showAll()}`);

  const done = createMemo(() => doneTodos(todos.all()).filter((todo) =>
    selectedWorkstream() !== null ? scope.showAll() || todo.home === selectedWorkstream() : isTodoOfRunning(todo, connected.rail.nodes)));

  const target = (todo: Todo) => ({ id: todo.id, label: `${todo.title} #${todo.id}` });

  const bulk = createBulkDelete({
    selected: selection.selected,
    targets: () => [...open(), ...(unfolded() ? done() : [])].map(target),
    done: () => done().map(target),
    context: () => `${connected.rail.selected()}:${scope.showAll()}`,
    noun: copy.todos,
    replace: selection.replace, clear: selection.clear, cancelSelection: selection.cancel,
    remove: (id) => failureOf(() => connected.app.rpc("todo.delete", { id })),
    reload: async () => {
      await todos.reload();

      return todos.failure();
    },
    reveal: () => setUnfolded(true),
  });

  const selectionKey = (todo: Todo, event: KeyboardEvent) => {
    bulk.key(event);

    if (!bulk.pending()) selection.key(todo.id, event);
  };

  const isEmpty = createMemo(() => todos.isLoaded() && todos.all().length === 0 && todos.failure() === null);
  const noneRunning = createMemo(() => selectedWorkstream() === null && todos.isLoaded() && todos.all().length > 0 && open().length === 0 && todos.failure() === null);
  const noneInWorkstream = createMemo(() => thisWorkstream() && todos.isLoaded() && todos.all().length > 0 && open().length === 0 && todos.failure() === null);

  const sorted = createSortedList({
    ids: () => open().map((todo) => todo.id),
    all: todos.all,
    call: (params) => failureOf(() => connected.app.rpc("todo.reorder", params)),
    reducedMotion: connected.reducedMotion,
    home: {
      target: (id, x, y) => {
        const current = todos.byId().get(id)?.home;
        const rail = document.querySelector(".rail");
        const clip = rail?.getBoundingClientRect();

        if (!clip || x < clip.left || x >= clip.right || y < clip.top || y >= clip.bottom) return null;

        for (const element of rail!.querySelectorAll<HTMLElement>(".rail-row[data-id], [data-todo-root]")) {
          const box = element.getBoundingClientRect();

          if (x < box.left || x >= box.right || y < box.top || y >= box.bottom) continue;
          const home = element.hasAttribute("data-todo-root") ? null : element.dataset.id!;

          if (home === current || (home !== null && !connected.rail.nodes.some((node) => node.id === home && node.kind === "workstream"))) return null;

          return { element, home };
        }

        return null;
      },
      move: (id, home) => failureOf(() => connected.app.rpc("todo.move", { id, home })),
    },
  });

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

    const home = selectedWorkstream();

    // U347: a Todo added while a Workstream is selected goes to that Workstream, so `this one` shows it.
    setCreateFailure(
      await failureOf(async () => {
        const made = await connected.app.rpc("todo.create", { title, body: null, blockers: null });

        if (home !== null) await connected.app.rpc("todo.move", { id: made.id, home });
      }),
    );
  };

  // U153: a row is selected while the Drawer it opened is the one open.
  const [shown, setShown] = createSignal<{ id: number; content: () => JSX.Element } | null>(null);
  const selectedId = () => (shown() !== null && connected.drawer.content() === shown()?.content ? shown()?.id : undefined);

  const show = (todo: Todo) => {
    const content = () => <TodoDrawer id={todo.id} todos={todos} />;

    setShown({ id: todo.id, content });
    connected.drawer.open(content);
  };

  const complete = async (todo: Todo) => {
    const target = rowAfter(todo.id, open().map((candidate) => candidate.id));
    const message = await failureOf(() => connected.app.rpc("todo.complete", { id: todo.id }));

    if (message === null) setAwaitingFocus((waiting) => [...waiting, { id: todo.id, target }]);

    return message;
  };

  return (
    <section aria-label={copy.todos}>
      <button
        data-button="add"
        type="button"
        aria-label={copy.addTodo}
        class="shelf-add"
        ref={(button) => (plusButton = button)}
        onClick={() => {
          setCreateFailure(null);
          setTyping(true);
        }}
      >
        <Icon name="plus" />
        <span>{copy.todo}</span>
      </button>
      <Show when={todos.failure()}>{(message) => <ErrorLine message={message()} />}</Show>
      <Show when={createFailure()}>{(message) => <ErrorLine message={message()} />}</Show>
      <Show when={typing()}>
        <input
          aria-label={copy.newTodoTitle}
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
        <p>{copy.noTodosYet}</p>
      </Show>
      <Show when={noneRunning()}>
        <p>{copy.noTodosFromRunningAgents}</p>
      </Show>
      <Show when={noneInWorkstream()}>
        <p>{copy.noTodosInThisWorkstream}{" "}
          <button data-button="quiet" type="button" onClick={() => scope.setShowAll(true)}>{copy.showAll}</button>
        </p>
      </Show>
      <Show when={open().length > 0}>
        <Sortable.Root class="todo-list shelf-list" plain ids={open().map((todo) => String(todo.id))} onOrderChange={sorted.order} external={sorted.external}>
        <For each={open()}>
          {(todo, at) => (
            <Sortable.Item id={String(todo.id)} index={at()} pressable>
              {(parts) => (
            <OpenRow
              todo={todo}
              selected={selection.selected().includes(todo.id) || selectedId() === todo.id}
              pressed={selection.selected().includes(todo.id)}
              selectionActive={selection.selected().length > 0}
              onPick={(item, event) => !bulk.pending() && selection.click(item.id, event, () => show(item))}
              onSelect={(item) => { if (!bulk.pending()) selection.toggle(item.id); }}
              onSelectionKey={selectionKey}
              known={todos.byId()}
              onOpen={show}
              onComplete={complete}
              sorted={sorted.row(todo.id, parts)}
              registerRow={(id, row) => {
                selection.register(id, row);

                if (row) rows.set(id, row);
                else rows.delete(id);
              }}
            />
              )}
            </Sortable.Item>
          )}
        </For>
        </Sortable.Root>
      </Show>
      <Show when={done().length > 0}>
        <div class="shelf-list">
          <ShelfRow glyph={<KindGlyph kind="done" decorative />} class="todo-inset">
            <div class="shelf-done-line">
              <RowButton hang={false} onClick={() => setUnfolded(!unfolded())}>
                <span class="shelf-title">{done().length}{copy.doneCount}</span>
              </RowButton>
              <button data-button="quiet" type="button" onClick={() => bulk.ask(true)}>{copy.clearDone}</button>
            </div>
          </ShelfRow>
          <Show when={unfolded()}>
            <For each={done()}>
              {(todo) => (
                <ShelfRow
                  glyph={<TickBox todo={todo} />}
                  class="todo-row todo-inset"
                  data-id={todo.id}
                  data-todo={todo.id}
                  data-selected={selection.selected().includes(todo.id) ? "true" : undefined}
                  head={
                    <RowButton
                      hang={false}
                      label={copy.todoRow(KIND_LABEL[kindOf(todo)], todo.id, todo.title)}
                      tooltip={todo.title}
                      pressed={selection.selected().includes(todo.id)}
                      selectionActive={selection.selected().length > 0}
                      ref={(row) => selection.register(todo.id, row)}
                      onKeyDown={(event) => selectionKey(todo, event)}
                      onClick={(event) => !bulk.pending() && selection.click(todo.id, event, () => show(todo))}
                    >
                      <span class="shelf-title">{todo.title}</span>
                    </RowButton>
                  }
                />
              )}
            </For>
          </Show>
        </div>
      </Show>
      <bulk.Bar />
    </section>
  );
};
