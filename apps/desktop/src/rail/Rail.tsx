import { createEffect, createMemo, createSignal, For, on, onCleanup, onMount, Show, untrack } from "solid-js";
import { ErrorLine } from "../ink/ErrorLine";
import { glyphOf } from "../ink/glyph";
import { useConnectedProject } from "../state/connectedProject";
import { attentionCount } from "./attention";
import { createRailDrag } from "./drag";
import { adjacentId } from "./keys";
import { ancestorsOf, layoutRail } from "./layout";
import type { NodeRow } from "./layout";
import { RailRowView } from "./RailRow";
import "./styles.css";

const toggled = <T,>(set: ReadonlySet<T>, member: T): ReadonlySet<T> =>
  new Set(set.has(member) ? [...set].filter((each) => each !== member) : [...set, member]);

export const Rail = () => {
  const { app, project, rail, now, reducedMotion, daemonExit } = useConnectedProject();

  const [collapsed, setCollapsed] = createSignal<ReadonlySet<string>>(new Set());
  const [unfolded, setUnfolded] = createSignal<ReadonlySet<string | null>>(new Set());
  const [failure, setFailure] = createSignal<string | null>(null);
  /** The Agent just spawned: `rail.tree` has no row for it until `rail.changed` is handled. */
  const [wanted, setWanted] = createSignal<string | null>(null);

  const [dragged, setDragged] = createSignal<string | null>(null);

  const rows = createMemo(() =>
    layoutRail(rail.nodes, { collapsed: collapsed(), unfolded: unfolded(), now: now(), dragged: dragged() }),
  );

  const nodeRows = createMemo(() => rows().filter((row): row is NodeRow => row.kind === "node"));
  const layoutKey = createMemo(() => nodeRows().map((row) => `${row.key}@${row.depth}`).join());
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

  const [container, setContainer] = createSignal<HTMLElement>();

  const rowElement = (id: string) => container()?.querySelector<HTMLElement>(`[data-id="${id}"]`);

  const focusRow = (id: string) => rowElement(id)?.focus();

  /** The row a keyboard-only move has focused, between an arrow press and the next `rail.select` call. */
  const [focusOverride, setFocusOverride] = createSignal<string | null>(null);

  createEffect(on(rail.selected, () => setFocusOverride(null)));

  const tabbableId = createMemo(() => {
    const ids = nodeRows().map((row) => row.node.id);
    const override = focusOverride();

    if (override !== null && ids.includes(override)) return override;

    return rail.selected() ?? ids[0] ?? null;
  });

  const selectRow = (id: string) => {
    rail.select(id);
    focusRow(id);
  };

  const moveFocus = (direction: 1 | -1) => {
    const next = adjacentId(nodeRows().map((row) => row.node.id), tabbableId(), direction);

    if (next !== null) {
      setFocusOverride(next);
      focusRow(next);
    }
  };

  const onRailKeyDown = (press: KeyboardEvent) => {
    if (!(press.target instanceof HTMLElement) || press.target.getAttribute("role") !== "treeitem") return;

    if (press.key === "ArrowDown" || press.key === "ArrowUp") {
      press.preventDefault();
      moveFocus(press.key === "ArrowDown" ? 1 : -1);
    } else if (press.key === "Enter") {
      press.preventDefault();

      const id = tabbableId();

      if (id !== null) selectRow(id);
    }
  };

  const drag = createRailDrag({
    container,
    nodes: () => rail.nodes,
    rows: nodeRows,
    layoutKey,
    onDragging: setDragged,
    enabled: () => daemonExit() === null,
    onDrop: (id, { parent, index }) => void attempt(() => app.rpc("rail.move", { id, parent, index })),
  });

  const [pending, setPending] = createSignal(false);

  /** Runs one of the three spawn actions; a second click while the Daemon is still answering would add a second row. */
  const guarded = <T,>(call: () => Promise<T>): void => {
    setPending(true);
    void attempt(call).finally(() => setPending(false));
  };

  const parent = (): string | null => {
    const selected = rail.nodes.find((node) => node.id === rail.selected());

    return selected?.kind === "group" ? selected.id : null;
  };

  const spawnAgent = () =>
    guarded(async () => {
      const spawned = await app.rpc("agent.spawn", { cwd: project.path, prompt: null, parent: parent() });

      setWanted(spawned.id);
    });

  const spawnTerminal = () => guarded(() => app.rpc("rail.spawnTerminal", { cwd: project.path, parent: parent() }));

  const canSpawn = () => !pending() && daemonExit() === null;

  const attention = createMemo(() => attentionCount(rail.nodes));

  createEffect(() => {
    void attempt(() => app.setDockBadge(attention()));
  });

  onMount(() => {
    const clear = () => setFailure(null);

    const chord = (press: KeyboardEvent) => {
      if (!press.metaKey || press.ctrlKey || press.altKey || press.shiftKey) return;

      const spawn = { n: spawnAgent, t: spawnTerminal }[press.key.toLowerCase()];

      if (!spawn) return;

      press.preventDefault();

      if (canSpawn()) spawn();
    };

    document.addEventListener("click", clear, true);
    document.addEventListener("keydown", chord);
    onCleanup(() => {
      document.removeEventListener("click", clear, true);
      document.removeEventListener("keydown", chord);
    });
  });

  /** A selection made elsewhere (the header's jump) may sit under a collapsed Group; its row is revealed and scrolled to. */
  createEffect(
    on(rail.selected, (id) => {
      if (id === null) return;

      const above = new Set(untrack(() => ancestorsOf(rail.nodes, id)));

      setCollapsed((closed) => new Set([...closed].filter((group) => !above.has(group))));
      queueMicrotask(() => rowElement(id)?.scrollIntoView({ block: "nearest" }));
    }),
  );

  createEffect(() => {
    const id = wanted();

    if (id !== null && rail.nodes.some((node) => node.id === id)) {
      rail.select(id);
      setWanted(null);
    }
  });

  return (
    <div
      class="rail-tree"
      ref={setContainer}
      style={{
        "--room": `${drag.state()?.room ?? 0}px`,
        "--lift": `${drag.state()?.lift ?? 0}px`,
        "--shift-ms": reducedMotion() ? "0ms" : "120ms",
      }}
    >
      <div
        role="tree"
        aria-label="rail"
        aria-disabled={daemonExit() !== null ? true : undefined}
        onKeyDown={onRailKeyDown}
      >
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
                          tabbable={tabbableId() === view().node.id}
                          now={now}
                          onSelect={() => selectRow(view().node.id)}
                          onToggle={() => setCollapsed((open) => toggled(open, view().node.id))}
                          onRename={(name) =>
                            void attempt(() => app.rpc("rail.rename", { id: view().node.id, name }))
                          }
                          onPromote={() => void attempt(() => app.rpc("rail.promote", { id: view().node.id }))}
                          dragging={dragged() !== null}
                          lifted={drag.state()?.lifted.has(view().node.id) ?? false}
                          shift={drag.state()?.shifts.get(view().node.id) ?? 0}
                          onPointerDown={(press) => drag.start(view().node.id, press)}
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
      <Show when={drag.state()}>
        {(dragging) => (
          <div
            class="drop-line"
            aria-hidden="true"
            style={{ top: `${dragging().top}px`, "--depth": dragging().drop.depth }}
          />
        )}
      </Show>
      <Show when={failure()}>{(message) => <ErrorLine message={message()} />}</Show>
      <div class="rail-actions">
        <button class="word" disabled={!canSpawn()} onClick={spawnAgent}>
          + agent
        </button>
        <button
          class="word"
          disabled={!canSpawn()}
          onClick={spawnTerminal}
        >
          + terminal
        </button>
        <button
          class="word"
          disabled={!canSpawn()}
          onClick={() => guarded(() => app.rpc("rail.createGroup", { name: "group", parent: parent() }))}
        >
          + group
        </button>
      </div>
    </div>
  );
};
