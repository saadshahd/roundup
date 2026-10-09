import { Icon } from "../ink/Icon";
import { createEffect, createSignal, For, on, Show } from "solid-js";
import type { Todo } from "@contracts/todo/Todo";
import { ErrorLine } from "../ink/ErrorLine";
import { KindGlyph } from "../ink/KindGlyph";
import { ItemDrawer } from "../drawer/ItemDrawer";
import { useConnectedProject } from "../state/connectedProject";
import { EditableText } from "./EditableText";
import { failureOf } from "./failureOf";
import { RowButton } from "./RowButton";
import type { TodosState } from "./state";
import { blockedBy, blockersOf, kindOf, offeredAsBlockers } from "./todoView";

const TodoLine = (props: { todo: Todo }) => (
  <p style={{ "white-space": "pre-wrap" }}>
    <KindGlyph kind={kindOf(props.todo)} /> {`#${props.todo.id}  ${props.todo.title}`}
  </p>
);

/** The Drawer of one Todo. What it shows follows the list, so an Event from the Daemon reaches it; `todo.get` is called once, on opening, by the shared item Drawer. */
export const TodoDrawer = (props: { id: number; todos: TodosState }) => {
  const { app, drawer, rail } = useConnectedProject();

  /** T8: `you` for the user, the Agent's name on the Rail, else the creator's id. */
  const creatorName = (creator: Todo["creator"]) =>
    creator.kind === "user" ? "you" : (rail.nodes.find((node) => node.kind === "agent" && node.id === creator.id)?.name ?? creator.id);

  const [failure, setFailure] = createSignal<string | null>(null);
  const [offering, setOffering] = createSignal(false);
  const [read, setRead] = createSignal(false);
  const [removedByMe, setRemovedByMe] = createSignal(false);
  const [asking, setAsking] = createSignal(false);
  let deleteButton: HTMLButtonElement | undefined;

  const attempt = async <Result,>(call: () => Promise<Result>) => {
    const message = await failureOf(call);

    setFailure(message);

    return message;
  };

  const todo = () => props.todos.byId().get(props.id);

  // Picks run one after another, each from the list the previous one answered: the list state only catches up when the Event arrives.
  const [answered, setAnswered] = createSignal<Todo | null>(null);
  let picks: Promise<unknown> = Promise.resolve();

  // The list state does not refire when only this Todo's fields change, so the cache watches the blockers itself.
  createEffect(on(() => todo()?.blockers.join(","), () => setAnswered(null), { defer: true }));

  const blockerBase = () => answered() ?? todo();

  const addBlocker = (blocker: number) => {
    const pick = () =>
      attempt(async () => {
        const base = blockerBase();

        if (!base) return;

        setAnswered(await app.rpc("todo.setBlockers", { id: props.id, blockers: [...base.blockers, blocker] }));
      });

    // A bug that rejects one pick is reported, and the picks behind it still run.
    picks = picks.then(pick, (bug) => {
      reportError(bug);

      return pick();
    });
  };

  return (
    <>
      <ItemDrawer item={`todo:${props.id}`} read={async () => {
          const fetched = await app.rpc("todo.get", { id: props.id });

          setRead(true);

          return fetched;
        }}>
        {() => (
          <Show
            when={todo()}
            fallback={
              <Show when={!removedByMe()}>
                <ErrorLine message={`#${props.id} was deleted`} />
              </Show>
            }
          >
            {(current) => (
              <>
                <div style={{ display: "flex", "white-space": "pre" }}>
                  <KindGlyph kind={kindOf(current())} />
                  <span>{` #${props.id}  `}</span>
                  <EditableText
                    name="title"
                    multiline={false}
                    value={current().title}
                    commit={(title) => attempt(() => app.rpc("todo.update", { id: props.id, title, body: null }))}
                  />
                </div>
                <Show when={blockersOf(current(), props.todos.byId()).length > 0}>
                  <p class="light">waits on</p>
                  <For each={blockersOf(current(), props.todos.byId())}>{(blocker) => <TodoLine todo={blocker} />}</For>
                </Show>
                <Show when={blockedBy(current(), props.todos.all()).length > 0}>
                  <p class="light">blocks</p>
                  <For each={blockedBy(current(), props.todos.all())}>{(blocked) => <TodoLine todo={blocked} />}</For>
                </Show>
                <p>
                  <button type="button" class="word" onClick={() => setOffering(!offering())}>
                    <Icon name="plus" /> blocker
                  </button>
                </p>
                <Show when={offering()}>
                  <For each={offeredAsBlockers(answered() ?? current(), props.todos.all())}>
                    {(other) => (
                      <RowButton
                        onClick={() => {
                          setOffering(false);
                          addBlocker(other.id);
                        }}
                      >
                        <KindGlyph kind={kindOf(other)} /> {`#${other.id}  ${other.title}`}
                      </RowButton>
                    )}
                  </For>
                </Show>
                <EditableText
                  name="body"
                  multiline
                  value={current().body}
                  commit={(body) => attempt(() => app.rpc("todo.update", { id: props.id, title: null, body }))}
                />
                <p class="light">created by {creatorName(current().creator)}</p>
              </>
            )}
          </Show>
        )}
      </ItemDrawer>
      <Show when={read() && todo()}>
        <Show
          when={asking()}
          fallback={
            <p>
              <button type="button" class="word light" onClick={() => void attempt(() => app.rpc("todo.complete", { id: props.id }))}>
                complete
              </button>{" "}
              <button
                type="button"
                class="word light"
                ref={(button) => (deleteButton = button)}
                onClick={() => {
                  setFailure(null);
                  setAsking(true);
                }}
              >
                delete
              </button>
            </p>
          }
        >
          <p class="light">
            {`delete #${props.id}?  `}
            <button
              type="button"
              class="word light"
              onClick={() =>
                void attempt(async () => {
                  await app.rpc("todo.delete", { id: props.id });
                  setRemovedByMe(true);
                  drawer.close();
                })
              }
            >
              delete
            </button>{" "}
            <button
              type="button"
              class="word light"
              ref={(button) => queueMicrotask(() => button.focus())}
              onClick={() => {
                setAsking(false);
                queueMicrotask(() => deleteButton?.focus());
              }}
            >
              keep
            </button>
          </p>
        </Show>
      </Show>
      <Show when={failure()}>{(message) => <ErrorLine message={message()} />}</Show>
    </>
  );
};
