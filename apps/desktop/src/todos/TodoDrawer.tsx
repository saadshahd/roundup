import { createSignal, For, Show } from "solid-js";
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
  const { app, drawer } = useConnectedProject();
  const [failure, setFailure] = createSignal<string | null>(null);
  const [offering, setOffering] = createSignal(false);

  const attempt = async <Result,>(call: () => Promise<Result>) => {
    const message = await failureOf(call);

    setFailure(message);

    return message;
  };

  const todo = () => props.todos.byId().get(props.id);

  return (
    <>
      <ItemDrawer item={`todo:${props.id}`} read={() => app.rpc("todo.get", { id: props.id })}>
        {() => (
          <Show when={todo()} fallback={<ErrorLine message={`#${props.id} was deleted`} />}>
            {(current) => (
              <>
                <div style={{ display: "flex", "white-space": "pre" }}>
                  <KindGlyph kind={kindOf(current())} /> <span>{`#${props.id}  `}</span>
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
                <button type="button" class="word" onClick={() => setOffering(!offering())}>
                  + blocker
                </button>
                <Show when={offering()}>
                  <For each={offeredAsBlockers(current(), props.todos.all())}>
                    {(other) => (
                      <RowButton
                        onClick={() => {
                          setOffering(false);
                          void attempt(() =>
                            app.rpc("todo.setBlockers", { id: props.id, blockers: [...current().blockers, other.id] }),
                          );
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
              </>
            )}
          </Show>
        )}
      </ItemDrawer>
      <Show when={todo()}>
        <p>
          <button type="button" class="word" onClick={() => void attempt(() => app.rpc("todo.complete", { id: props.id }))}>
            complete
          </button>{" "}
          <button
            type="button"
            class="word"
            onClick={() =>
              void attempt(async () => {
                await app.rpc("todo.delete", { id: props.id });
                drawer.close();
              })
            }
          >
            delete
          </button>
        </p>
      </Show>
      <Show when={failure()}>{(message) => <ErrorLine message={message()} />}</Show>
    </>
  );
};
