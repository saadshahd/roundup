import { createMemo, createSignal, For, Show } from "solid-js";
import type { Accessor } from "solid-js";
import type { RailNode } from "@contracts/agent/RailNode";
import { ErrorLine } from "../../ink/ErrorLine";
import { Icon } from "../../ink/Icon";
import { OwnerMark } from "../../ink/OwnerMark";
import { PadDrawer } from "../../pads/PadDrawer";
import { cutName } from "../../pads/nameLine";
import { useConnectedProject } from "../../state/connectedProject";
import { OpenRow } from "../../todos/Todos";
import { failureOf } from "../../todos/failureOf";
import { TodoDrawer } from "../../todos/TodoDrawer";
import { createReorder } from "../../todos/reorder";
import { useTodoList } from "../../todos/todoList";
import { openTodosBy, padsOwnedBy } from "./owned";
import { usePadList } from "./padList";

/** The Pads and open Todos under one Rail row (U58, U60): `n` is how many its Agent owns and created together (0 for any other row), `shown` is whether the control is open. */
export const createAgentPads = (node: Accessor<RailNode>) => {
  const all = usePadList().pads;
  const todos = useTodoList();
  const [shown, setShown] = createSignal(false);
  const [failure, setFailure] = createSignal<string | null>(null);
  const owned = createMemo(() => (node().kind === "agent" ? padsOwnedBy(all(), node().id) : []));
  const ownedTodos = createMemo(() => (node().kind === "agent" ? openTodosBy(todos.all(), node().id) : []));
  const connected = useConnectedProject();

  const reorder = createReorder({
    ids: () => ownedTodos().map((todo) => todo.id),
    all: todos.all,
    call: (params) => failureOf(() => connected.app.rpc("todo.reorder", params)),
    reducedMotion: connected.reducedMotion,
  });

  const count = () => owned().length + ownedTodos().length;

  return {
    all,
    owned,
    todos,
    ownedTodos,
    reorder,
    n: count,
    shown: () => shown() && count() > 0,
    toggle: () => setShown((open) => !open),
    failure,
    setFailure,
  };
};

export type AgentPads = ReturnType<typeof createAgentPads>;

/** `◈ <n>` at the right end of an Agent's row; absent at 0. */
export const PadsControl = (props: { pads: AgentPads }) => (
  <Show when={props.pads.n() > 0}>
    <button
      class="word rail-pads-control"
      data-pads-control
      aria-label={`${props.pads.n()} pads`}
      aria-expanded={props.pads.shown()}
      onClick={(click) => {
        click.stopPropagation();
        props.pads.toggle();
      }}
    >
      <Icon name="owned" /> {props.pads.n()}
    </button>
  </Show>
);

/** The Pad rows, then the open Todo rows, one level below `depth`, folded at rest; a click opens a Pad's Drawer as the Shelf's list does (U20). Opening changes no selection (U64). */
export const PadRows = (props: { pads: AgentPads; depth: number }) => {
  const connected = useConnectedProject();

  return (
    <>
      <Show when={props.pads.shown()}>
        <div class="rail-pads" data-pads role="group" style={{ "padding-left": `${(props.depth + 1) * 2}ch` }}>
          <For each={props.pads.owned()}>
            {(pad) => (
              <button
                type="button"
                class="rail-pad"
                title={pad.name}
                onClick={() => {
                  props.pads.setFailure(null);
                  connected.drawer.open(() => (
                    <PadDrawer name={pad.name} pads={props.pads.all} onLateFailure={props.pads.setFailure} />
                  ));
                }}
              >
                <OwnerMark owner={pad.owner} decorative />
                <span style={cutName}>{pad.name}</span>
              </button>
            )}
          </For>
          <For each={props.pads.ownedTodos()}>
            {(todo) => (
              <OpenRow
                nested
                todo={todo}
                known={props.pads.todos.byId()}
                onOpen={(opened) => connected.drawer.open(() => <TodoDrawer id={opened.id} todos={props.pads.todos} />)}
                reorder={props.pads.reorder}
                onComplete={(done) => failureOf(() => connected.app.rpc("todo.complete", { id: done.id }))}
              />
            )}
          </For>
        </div>
      </Show>
      <Show when={props.pads.failure()}>{(message) => <ErrorLine message={message()} />}</Show>
    </>
  );
};
