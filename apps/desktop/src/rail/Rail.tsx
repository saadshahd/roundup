import { createEffect, createMemo, createSignal, For, Show } from "solid-js";
import { ErrorLine } from "../ink/ErrorLine";
import { glyphOf } from "../ink/glyph";
import { useConnectedProject } from "../state/connectedProject";
import { attentionCount } from "./attention";
import { layoutRail } from "./layout";
import { RailRowView } from "./RailRow";
import "./rail.css";

const toggled = <T,>(set: ReadonlySet<T>, member: T): ReadonlySet<T> =>
  new Set(set.has(member) ? [...set].filter((each) => each !== member) : [...set, member]);

export const Rail = () => {
  const { app, project, rail, now } = useConnectedProject();

  const [collapsed, setCollapsed] = createSignal<ReadonlySet<string>>(new Set());
  const [unfolded, setUnfolded] = createSignal<ReadonlySet<string | null>>(new Set());
  const [failure, setFailure] = createSignal<string | null>(null);
  /** The Agent just spawned: `rail.tree` has no row for it until `rail.changed` is handled. */
  const [wanted, setWanted] = createSignal<string | null>(null);

  const rows = createMemo(() => layoutRail(rail.nodes, { collapsed: collapsed(), unfolded: unfolded(), now: now() }));
  const keys = createMemo(() => rows().map((row) => row.key));
  const byKey = createMemo(() => new Map(rows().map((row) => [row.key, row])));

  const attempt = async <T,>(call: () => Promise<T>): Promise<void> => {
    try {
      await call();
    } catch (error) {
      if (!(error instanceof Error)) throw error;

      setFailure(error.message);
    }
  };

  const parent = (): string | null => {
    const selected = rail.nodes.find((node) => node.id === rail.selected());

    return selected && (selected.kind === "group" || selected.meta) ? selected.id : null;
  };

  createEffect(() => {
    void attempt(() => app.setDockBadge(attentionCount(rail.nodes)));
  });

  createEffect(() => {
    const id = wanted();

    if (id !== null && rail.nodes.some((node) => node.id === id)) {
      rail.select(id);
      setWanted(null);
    }
  });

  return (
    <div class="rail-tree" onClick={() => setFailure(null)}>
      <div role="tree" aria-label="rail">
        <For each={keys()}>
          {(key) => (
            <Show when={byKey().get(key)}>
              {(row) => {
                const node = createMemo(() => {
                  const current = row();

                  return current.kind === "node" ? current : undefined;
                });

                const fold = createMemo(() => {
                  const current = row();

                  return current.kind === "fold" ? current : undefined;
                });

                return (
                  <>
                    <Show when={node()}>
                      {(view) => (
                        <RailRowView
                          row={view()}
                          exit={rail.exitOf(view().node)}
                          selected={rail.selected() === view().node.id}
                          now={now}
                          onSelect={() => rail.select(view().node.id)}
                          onToggle={() => setCollapsed((open) => toggled(open, view().node.id))}
                          onRename={(name) =>
                            void attempt(() => app.rpc("rail.rename", { id: view().node.id, name }))
                          }
                          onPromote={() => void attempt(() => app.rpc("rail.promote", { id: view().node.id }))}
                        />
                      )}
                    </Show>
                    <Show when={fold()}>
                      {(view) => (
                        <div style={{ "padding-left": `${view().depth * 2}ch` }}>
                          <button
                            class="word glyph"
                            data-tone={glyphOf("done").tone}
                            onClick={() => setUnfolded((open) => toggled(open, view().parent))}
                          >
                            {`${glyphOf("done").mark} ${view().count} done`}
                          </button>
                        </div>
                      )}
                    </Show>
                  </>
                );
              }}
            </Show>
          )}
        </For>
      </div>
      <Show when={failure()}>{(message) => <ErrorLine message={message()} />}</Show>
      <div class="rail-actions">
        <button
          class="word"
          onClick={() =>
            void attempt(async () => {
              const spawned = await app.rpc("agent.spawn", { cwd: project.path, prompt: null, parent: parent() });

              setWanted(spawned.id);
            })
          }
        >
          + agent
        </button>
        <button
          class="word"
          onClick={() => void attempt(() => app.rpc("rail.spawnTerminal", { cwd: project.path, parent: parent() }))}
        >
          + terminal
        </button>
        <button
          class="word"
          onClick={() => void attempt(() => app.rpc("rail.createGroup", { name: "group", parent: parent() }))}
        >
          + group
        </button>
      </div>
    </div>
  );
};
