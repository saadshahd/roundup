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

/** The Drawer of one Todo. What it shows follows the list, so an Event from the Daemon reaches it; `todo.get` is called once, on opening. */
export const TodoDrawer = (props: { id: number; todos: TodosState }) => {
  const { app, drawer } = useConnectedProject();
  const [failure, setFailure] = createSignal<string | null>(null);
  const [offering, setOffering] = createSignal(false);

  const attempt = async <Result,>(call: () => Promise<Result>) => setFailure(await failureOf(call));

  return (
    <ItemDrawer item={`todo:${props.id}`} read={() => app.rpc("todo.get", { id: props.id })}>
      {(fetched) => {
        const todo = () => props.todos.all().find((other) => other.id === props.id) ?? fetched;
        const waitsOn = () => blockersOf(todo(), props.todos.all());
        const blocks = () => blockedBy(todo(), props.todos.all());

        return (
          <>
            <div style={{ display: "flex", "white-space": "pre" }}>
              <KindGlyph kind={kindOf(todo())} /> <span>{`#${props.id}  `}</span>
              <EditableText
                name="title"
                multiline={false}
                value={todo().title}
                onCommit={(title) => void attempt(() => app.rpc("todo.update", { id: props.id, title, body: null }))}
              />
            </div>
            <Show when={waitsOn().length > 0}>
              <p class="light">waits on</p>
              <For each={waitsOn()}>{(blocker) => <TodoLine todo={blocker} />}</For>
            </Show>
            <Show when={blocks().length > 0}>
              <p class="light">blocks</p>
              <For each={blocks()}>{(blocked) => <TodoLine todo={blocked} />}</For>
            </Show>
            <button type="button" class="word" onClick={() => setOffering(!offering())}>
              + blocker
            </button>
            <Show when={offering()}>
              <For each={offeredAsBlockers(todo(), props.todos.all())}>
                {(other) => (
                  <RowButton
                    onClick={() => {
                      setOffering(false);
                      void attempt(() =>
                        app.rpc("todo.setBlockers", { id: props.id, blockers: [...todo().blockers, other.id] }),
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
              value={todo().body}
              onCommit={(body) => void attempt(() => app.rpc("todo.update", { id: props.id, title: null, body }))}
            />
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
            <Show when={failure()}>{(message) => <ErrorLine message={message()} />}</Show>
          </>
        );
      }}
    </ItemDrawer>
  );
};
