import { createEffect, createMemo, createSignal, For, on, onCleanup, onMount, Show } from "solid-js";
import { ErrorLine } from "../ink/ErrorLine";
import { KindGlyph } from "../ink/KindGlyph";
import { Icon } from "../ink/Icon";
import { useConnectedProject } from "../state/connectedProject";
import { attentionCount } from "./attention";
import { createRailDrag } from "./drag";
import { adjacentId } from "./keys";
import { RailMenu } from "./menu/RailMenu";
import { createRailMenu } from "./menu/model";
import { layoutRail, resolvedWorkstream } from "./layout";
import type { NodeRow } from "./layout";
import { RailRowView } from "./RailRow";
import { SpawnPromptField } from "./SpawnPromptField";
import "./styles.css";

const toggled = <T,>(set: ReadonlySet<T>, member: T): ReadonlySet<T> =>
  new Set(set.has(member) ? [...set].filter((each) => each !== member) : [...set, member]);

export const Rail = () => {
  const { app, project, rail, now, reducedMotion, daemonExit } = useConnectedProject();

  const collapsed = rail.collapsed;
  const [unfolded, setUnfolded] = createSignal<ReadonlySet<string | null>>(new Set());
  const [failure, setFailure] = createSignal<string | null>(null);
  const menu = createRailMenu(app, rail, setFailure);
  const [wanted, setWanted] = createSignal<string | null>(null);
  /** Gates `⌘N` in the chord handler and the `+ agent` click below, so an open field is never dropped mid-type (U33). */
  const [composing, setComposing] = createSignal(false);

  const [dragged, setDragged] = createSignal<string | null>(null);

  const rows = createMemo(() =>
    layoutRail(rail.nodes, { collapsed: collapsed(), unfolded: unfolded(), now: now(), dragged: dragged() }),
  );

  /** U38: the empty line waits for the first `rail.tree` (always answered before the Rail mounts) and yields to a fetch failure, which the centre shows instead. */
  const isEmpty = createMemo(() => rail.nodes.length === 0 && rail.failure() === null);

  const nodeRows = createMemo(() => rows().filter((row): row is NodeRow => row.kind === "node"));
  const nodeIds = createMemo(() => nodeRows().map((row) => row.node.id));
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
  const [field, setField] = createSignal<HTMLInputElement>();

  const rowElement = (id: string) => container()?.querySelector<HTMLElement>(`[data-id="${id}"]`);

  const focusRow = (id: string) => rowElement(id)?.focus();

  const tabbableId = createMemo(() => {
    const selected = rail.selected();
    const ids = nodeIds();

    return ids.find((id) => id === selected) ?? ids[0] ?? null;
  });

  const moveFocus = (current: string, direction: 1 | -1) => focusRow(adjacentId(nodeIds(), current, direction));

  const onRailKeyDown = (press: KeyboardEvent) => {
    const focusedId = press.target instanceof HTMLElement ? press.target.dataset.id : undefined;

    if (focusedId === undefined) return;

    if (press.key === "ContextMenu" || (press.key === "F10" && press.shiftKey)) {
      press.preventDefault();
      const row = rowElement(focusedId);
      const node = rail.nodes.find((candidate) => candidate.id === focusedId);

      if (row && node) {
        const box = row.getBoundingClientRect();
        menu.open(node, rail.exitOf(node), box.left, box.bottom);
      }

      return;
    }

    if (press.key === "ArrowDown" || press.key === "ArrowUp") {
      press.preventDefault();
      moveFocus(focusedId, press.key === "ArrowDown" ? 1 : -1);
    } else if (press.key === "Enter") {
      press.preventDefault();
      rail.select(focusedId);
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

  /** Runs one of the add actions; a second click while the Daemon is still answering would add a second row. */
  const guarded = <T,>(call: () => Promise<T>): void => {
    setPending(true);
    void attempt(call).finally(() => setPending(false));
  };

  const parent = (): string | null => {
    const selected = rail.nodes.find((node) => node.id === rail.selected());

    return selected?.kind === "workstream" ? selected.id : null;
  };

  /** U162: the Workstream whose group holds `+ agent  + terminal`: the selected Workstream, else the one above the selected row. */
  const resolved = createMemo(() => resolvedWorkstream(rail.nodes, rail.selected()));

  /** The key of the last row of the resolved Workstream's group (`null` when it is collapsed or none resolves): the inside line follows it. */
  const addsAfter = createMemo(() => {
    const id = resolved();
    const all = rows();
    const start = all.findIndex((row) => row.kind === "node" && row.node.id === id);
    const head = all[start];

    if (head?.kind !== "node" || head.collapsed !== null) return null;

    const end = all.findIndex((row, index) => index > start && row.depth <= head.depth);

    return { key: all[(end === -1 ? all.length : end) - 1]!.key, depth: head.depth + 1, workstream: head.node.id };
  });

  const spawnAgentWith = (prompt: string | null, under: string | null = parent()) =>
    guarded(async () => {
      const spawned = await app.rpc("agent.spawn", { cwd: project.path, prompt, parent: under });

      setWanted(spawned.id);
      setComposing(false);
    });

  const spawnAgent = () => spawnAgentWith(null);

  const spawnTerminalUnder = (under: string | null) =>
    guarded(async () => {
      const spawned = await app.rpc("rail.spawnTerminal", { cwd: project.path, parent: under });

      setWanted(spawned.id);
    });

  const spawnTerminal = () => spawnTerminalUnder(parent());

  const canSpawn = () => !pending() && !rail.workstreamCreating() && !rail.doorPending(rail.selected() ?? "") && daemonExit() === null;

  const attention = createMemo(() => attentionCount(rail.nodes));

  createEffect(() => {
    void attempt(() => app.setDockBadge(attention()));
  });

  onMount(() => {
    const clear = () => {
      setFailure(null);
      rail.clearWorkstreamFailure();
      rail.clearStorageFailure();
    };

    const chord = (press: KeyboardEvent) => {
      if (!press.metaKey || press.ctrlKey || press.altKey) return;

      const key = press.key.toLowerCase();

      if (press.shiftKey) {
        if (key !== "n") return;

        press.preventDefault();

        if (press.repeat) return;

        if (canSpawn()) setComposing(true);

        return;
      }

      const spawn = { n: spawnAgent, t: spawnTerminal }[key];

      if (!spawn) return;

      press.preventDefault();

      if (press.repeat) return;

      if (key === "n" && composing()) return;

      if (canSpawn()) spawn();
    };

    document.addEventListener("click", clear, true);
    document.addEventListener("keydown", chord);
    onCleanup(() => {
      document.removeEventListener("click", clear, true);
      document.removeEventListener("keydown", chord);
    });
  });

  /** Restoring must not scroll before the window has placed focus (U100). */
  createEffect(
    on(rail.selected, (id) => {
      if (id === null || rail.restored()) return;

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
      <Show when={composing()}>
        <SpawnPromptField
          ref={setField}
          onSubmit={(prompt) => {
            if (canSpawn()) spawnAgentWith(prompt);
          }}
          onCancel={() => setComposing(false)}
        />
      </Show>
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
                          onSelect={() => rail.select(view().node.id)}
                          onToggle={() => rail.toggleCollapsed(view().node.id)}
                          onRename={(name) =>
                            void attempt(() => app.rpc("rail.rename", { id: view().node.id, name }))
                          }
                          onStartDoor={() => { rail.select(view().node.id); void rail.startDoor(view().node.id); }}
                          doorPending={rail.doorPending(view().node.id)}
                          doorFailure={rail.doorFailure(view().node.id)}
                          dragging={dragged() !== null}
                          lifted={drag.state()?.lifted.has(view().node.id) ?? false}
                          shift={drag.state()?.shifts.get(view().node.id) ?? 0}
                          onPointerDown={(press) => drag.start(view().node.id, press)}
                          onOpenMenu={(press) => menu.open(view().node, rail.exitOf(view().node), press.clientX, press.clientY)}
                        />
                      )}
                    </Show>
                    <Show when={fold()}>
                      {(view) => (
                        <div class="rail-fold" style={{ "padding-left": `${view().depth * 2}ch` }}>
                          <button
                            class="word"
                            onClick={() => setUnfolded((open) => toggled(open, view().parent))}
                          >
                            <KindGlyph kind="done" decorative /> {view().count} done
                          </button>
                        </div>
                      )}
                    </Show>
                    <Show when={addsAfter()?.key === key ? addsAfter() : undefined}>
                      {(adds) => (
                        <div class="rail-adds" style={{ "padding-left": `${adds().depth * 2}ch` }}>
                          <button
                            class="word"
                            disabled={!canSpawn()}
                            aria-disabled={canSpawn() ? undefined : true}
                            onClick={() => (composing() ? field()?.focus() : spawnAgentWith(null, adds().workstream))}
                          >
                            <Icon name="plus" /> agent
                          </button>
                          <button
                            class="word"
                            disabled={!canSpawn()}
                            aria-disabled={canSpawn() ? undefined : true}
                            onClick={() => spawnTerminalUnder(adds().workstream)}
                          >
                            <Icon name="plus" /> terminal
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
      <Show when={isEmpty()}>
        <p>no agents yet</p>
        <p>⌘N starts one</p>
      </Show>
      <Show when={drag.state()}>
        {(dragging) => (
          <div
            class="drop-line"
            aria-hidden="true"
            style={{ top: `${dragging().top}px`, "--depth": dragging().drop.depth }}
          />
        )}
      </Show>
      <Show when={failure() ?? rail.workstreamFailure() ?? rail.doorFailure(rail.selected() ?? "") ?? rail.storageFailure()}>{(message) => <ErrorLine message={message()} />}</Show>
      <RailMenu menu={menu} />
      <div class="rail-actions">
        <button
          class="word"
          disabled={!canSpawn()}
          aria-disabled={canSpawn() ? undefined : true}
          onClick={() => guarded(() => rail.createWorkstream())}
        >
          <Icon name="plus" /> new Workstream
        </button>
      </div>
    </div>
  );
};
