import { captureFocus } from "../keys/focus";
import { writeText } from "../app/clipboard";
import * as v from "valibot";
import { attentionCount } from "./attention";
import { copy } from "../copy";
import { createEffect, createMemo, createSignal, For, on, onCleanup, onMount, Show } from "solid-js";
import { Portal } from "solid-js/web";
import { createItemFailure } from "./itemFailure";
import { ReservedLine } from "../ink/ReservedLine";
import { SectionHead } from "../ink/SectionHead";
import { KindGlyph } from "../ink/KindGlyph";
import { Icon } from "../ink/Icon";
import { CHORDS } from "../keys/chords";
import { registerKeys } from "../keys/registry";
import { Sortable, type SortableMove } from "../ui/sortable";
import { TreeView } from "../ui/TreeView";
import { railCollection } from "./collection";
import { useConnectedProject } from "../state/connectedProject";
import { doorStateOf } from "./doorState";
import { RailConfirm } from "./menu/RailConfirm";
import { createRailMenu } from "./menu/model";
import { agentKindsBelow, isWorkstream, layoutRail, siblingsOf } from "./layout";
import type { NodeRow } from "./layout";
import { RailRowView } from "./RailRow";
import { focusedRowId, ROW } from "./rows";
import { Trunks } from "./Trunks";
import { SpawnPromptField } from "./SpawnPromptField";
import type { RailNode } from "@contracts/agent/RailNode";
import type { ExitState } from "../state/rail";
import type { RowActions } from "./RowIcons";
import type { SpawnParams } from "@contracts/agent/SpawnParams";
import { ClearPrompt } from "./ClearPrompt";
import { INTERNAL_CODE, RpcError } from "../app/seam";
import { clearingOf } from "./clear";
import { useRecoveryMenu } from "../drawer/DraftRecovery";
import { contextMenu, openOnRelease } from "../ui/contextMenu";
import { draftsOf } from "./persist/storage";
import "./styles.css";

const toggled = <T,>(set: ReadonlySet<T>, member: T): ReadonlySet<T> =>
  new Set(set.has(member) ? [...set].filter((each) => each !== member) : [...set, member]);

export const Rail = () => {
  const connected = useConnectedProject();
  const { app, project, rail, now, daemonExit } = connected;
  const recoveryEntries = useRecoveryMenu();

  const attention = createMemo(() => attentionCount(rail.nodes));
  const [scrolled, setScrolled] = createSignal(false);
  const collapsed = rail.collapsed;
  const [unfolded, setUnfolded] = createSignal<ReadonlySet<string | null>>(new Set());
  const failure = createItemFailure((id) => rail.nodes.some((node) => node.id === id));
  const retained = draftsOf(connected.drafts);
  // #2788: avoid rescanning all drafts per recovery row; 10k lookups took 527 ms with find, 0.6 ms with Map.
  const revisions = createMemo(() => new Map(retained.records().map((draft) => [draft.revision, draft])));

  const menu = createRailMenu(app, rail, failure.show, failure.acknowledge, connected.events, connected.drafts, (id) => {
    rowElement(id)?.querySelector(".name")?.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
  }, connected.queue);


  const clearPrompt = createMemo(() => {
    const node = menu.clearTarget();

    return node !== null && clearingOf(app, connected.events, node.id, connected.drafts).phase() !== "closed" ? node : null;
  });

  const [wanted, setWanted] = createSignal<{ id: string; focus: ReturnType<typeof captureFocus> } | null>(null);
  /** Gates `⌘N` in the chord handler and the add-agent click below, so an open field is never dropped mid-type (U33). */
  const [composing, setComposing] = createSignal(false);
  const [editingName, setEditingName] = createSignal<string | null>(null);
  const [taskDestination, setTaskDestination] = createSignal<string | null>(null);
  const task = () => retained.records().filter((entry) => entry.destination.surface === "rail-task").at(-1);

  const recovery = createMemo(() => retained.records().filter((entry) => {
    const { surface, object } = entry.destination;

    if (!surface.startsWith("rail-")) return false;

    if (surface === "rail-task" && composing() && entry.revision === task()?.revision) return false;

    if (surface === "rail-name" && object === editingName()) {
      return entry.revision !== retained.records().filter((draft) => draft.destination.surface === surface && draft.destination.object === object).at(-1)?.revision;
    }

    if (surface === "rail-clear" && object === clearPrompt()?.id) {
      return entry.revision !== retained.records().filter((draft) => draft.destination.surface === surface && draft.destination.object === object).at(-1)?.revision;
    }

    return true;
  }));

  const destinationName = (id: string) => id === "" || id === project.path ? copy.overview : rail.nodes.find((node) => node.id === id)?.name ?? copy.railDraftMissingTarget;

  const draft = () => {
    const text = task()?.value.text;

    return v.is(v.string(), text) ? text : "";
  };

  const taskParent = () => {
    const saved = task();

    if (!saved) return taskDestination();

    if (v.is(v.nullable(v.string()), saved.value.parent)) return saved.value.parent;

    return saved.destination.object === "" || saved.destination.object === project.path ? null : saved.destination.object;
  };

  const keepDraft = (text: string, selection = task()?.selection ?? null) => {
    const under = taskParent();
    const action = taskAction();

    return retained.edit(
      task()?.destination ?? { surface: "rail-task", object: under ?? project.path, recipient: action === "note" ? under : null },
      { text, parent: under, model: null, effort: null, action }, selection, "ordinary",
    );
  };

  const taskAction = () => {
    const saved = task();

    if (saved) return v.is(v.picklist(["note", "spawn"]), saved.value.action) ? saved.value.action : saved.destination.recipient === null ? "spawn" : "note";

    const original = rail.nodes.find((node) => node.id === taskDestination());

    return original !== undefined && doorStateOf(original, rail.exitOf(original), rail.doorPending(original.id), rail.doorFailure(original.id) !== null) === null ? "note" : "spawn";
  };

  const openCompose = () => {
    captureFocus(true);

    if (!composing()) setTaskDestination(parent());
    setComposing(true);
  };

  const endCompose = () => setComposing(false);

  const rows = createMemo(() => layoutRail(rail.nodes, { collapsed: collapsed(), unfolded: unfolded(), now: now() }));

  /** U38: the empty line waits for the first `rail.tree` (always answered before the Rail mounts) and yields to a fetch failure, which the centre shows instead. */

  /** U520: every Workstream open, so Ark can open one that is folded and walk into it; Ark's `expandedValue` keeps the folded ones shut. */
  const collection = createMemo(() => railCollection(layoutRail(rail.nodes, { collapsed: new Set(), unfolded: unfolded(), now: now() })));
  const expanded = createMemo(() => collection().getBranchValues().filter((id) => !collapsed().has(id)));

  /** The tree node and index path Ark's providers need for the row `id`. */
  const entryOf = (id: string) => {
    const indexPath = collection().getIndexPath(id);
    const node = indexPath && collection().at(indexPath);

    if (!indexPath || !node) throw new Error(`no node for row ${id}`);

    return { node, indexPath };
  };

  const nodeRows = createMemo(() => rows().filter((row): row is NodeRow => row.kind === "node"));
  const nodeIds = createMemo(() => nodeRows().map((row) => row.node.id));
  const indexOf = createMemo(() => new Map(nodeIds().map((id, at) => [id, at])));
  const treeRows = createMemo(() => nodeRows().map((row) => ({ id: row.node.id, parent: row.node.parent })));
  const keys = createMemo(() => rows().map((row) => row.key));
  const byKey = createMemo(() => new Map(rows().map((row) => [row.key, row])));

  const attempt = async <T,>(call: () => Promise<T>, item?: string): Promise<void> => {
    try {
      await call();

      if (item !== undefined) failure.acknowledge(item);
    } catch (error) {
      if (!(error instanceof Error)) throw error;

      failure.show(error.message, item);
    }
  };

  const [container, setContainer] = createSignal<HTMLElement>();
  const [field, setField] = createSignal<HTMLTextAreaElement>();

  /** The focusable element of the row `id`: its item, or the control of its branch. */
  const rowElement = (id: string) => container()?.querySelector<HTMLElement>(`[data-id="${CSS.escape(id)}"]`)?.closest<HTMLElement>(ROW) ?? container()?.querySelector<HTMLElement>(`[data-id="${CSS.escape(id)}"] [data-part="branch-control"]`) ?? undefined;

  let clearElement: HTMLDivElement | undefined;

  /** U350: the prompt stays within the Rail, so its ground never runs into the centre pane's ground and reads as cut off. */
  const placeClear = () => {
    const node = clearPrompt();
    const row = node ? rowElement(node.id)?.getBoundingClientRect() : undefined;
    const rail = container()?.getBoundingClientRect();

    if (!row || !rail || !clearElement) return;

    const inset = parseFloat(getComputedStyle(clearElement).paddingLeft);

    // The box is content-box: its width leaves out its own padding, which equals the inset kept on each side.
    clearElement.style.maxWidth = `${Math.max(0, rail.width - 4 * inset)}px`;

    const box = clearElement.getBoundingClientRect();
    const top = row.bottom + box.height <= window.innerHeight - inset ? row.bottom : row.top - box.height;

    clearElement.style.left = `${Math.max(rail.left + inset, Math.min(row.left, rail.right - box.width - inset))}px`;
    clearElement.style.top = `${Math.max(inset, Math.min(top, window.innerHeight - box.height - inset))}px`;
  };

  onMount(() => {
    window.addEventListener("resize", placeClear);
    document.addEventListener("scroll", placeClear, true);
    onCleanup(() => {
      window.removeEventListener("resize", placeClear);
      document.removeEventListener("scroll", placeClear, true);
    });
  });

  const focusRow = (id: string) => rowElement(id)?.focus();

  /** U31: exactly one row is reachable by Tab, the selected row when one exists, else the first; moving focus by key does not move it. */
  const tabbableId = createMemo(() => {
    const selected = rail.selected();
    const ids = nodeIds();

    return ids.find((id) => id === selected) ?? ids[0] ?? null;
  });

  /** U358: `⌘⌫` in the Rail takes the row menu's `remove` for the selected row; the selection then moves to the next row, else the previous, and focus stays in the Rail. A repeat acts on nothing. */
  const removeSelected = (repeat: boolean) => {
    const id = rail.selected();
    const node = rail.nodes.find((candidate) => candidate.id === id);
    const row = id === null ? undefined : rowElement(id);

    if (repeat || !node || !row) return;

    const ids = nodeIds();
    const next = ids[ids.indexOf(node.id) + 1] ?? ids[ids.indexOf(node.id) - 1];
    menu.requestRemove(node, rail.exitOf(node), () => {
      if (next === undefined || rail.selected() !== node.id) return;

      rail.select(next);
      focusRow(next);
    });
  };

  const openMenuAtFocusedRow = (press: KeyboardEvent) => {
    const id = focusedRowId(press);
    const row = id === undefined ? undefined : rowElement(id);
    const node = rail.nodes.find((candidate) => candidate.id === id);

    if (row && node) {
      const box = row.getBoundingClientRect();
      rail.select(node.id);
      void menu.open(node, rail.exitOf(node), box.left, box.bottom);
    }
  };

  const openOverviewMenu = (press: KeyboardEvent) => {
    const entries = recoveryEntries("key");

    if (entries.length === 0 || !(press.target instanceof HTMLElement)) return;

    const box = press.target.getBoundingClientRect();
    void contextMenu(app, entries, box.left, box.bottom);
  };

  /** One indent step (--rail-step) in pixels: how far sideways the pointer goes to change the drop's depth. */
  const [railStep, setRailStep] = createSignal(1);

  const [drop, setDrop] = createSignal<{ depth: number; top: number } | null>(null);

  /** Whether the press that began the drag was a pointer's: a keyboard lift has no pointer to leave the Rail. */
  let pointerPressed = false;
  let pointerAt: { x: number; y: number } | null = null;
  /** The row a keyboard lift began on: the library returns focus only to a handle, and the whole row is the Rail's. */
  let liftedRow: string | null = null;

  const pointerOutside = () => {
    const box = container()?.getBoundingClientRect();

    return pointerPressed && pointerAt !== null && box !== undefined &&
      (pointerAt.x < box.left || pointerAt.x >= box.right || pointerAt.y < box.top || pointerAt.y >= box.bottom);
  };

  onMount(() => {
    const tree = container();

    if (tree) {
      // --rail-step is a calc(), so a probe resolves it to pixels once.
      const probe = tree.appendChild(document.createElement("span"));
      probe.style.display = "inline-block";
      probe.style.width = "var(--rail-step)";
      const width = probe.getBoundingClientRect().width;
      probe.remove();

      if (width > 0) setRailStep(width);
    }

    const press = (event: PointerEvent) => {
      pointerPressed = true;
      pointerAt = { x: event.clientX, y: event.clientY };
    };

    const follow = (event: PointerEvent) => { pointerAt = { x: event.clientX, y: event.clientY }; };

    const lift = (key: KeyboardEvent) => {
      pointerPressed = false;

      if (key.code === "Space" && key.target instanceof HTMLElement && key.target.matches(ROW)) liftedRow = key.target.closest<HTMLElement>("[data-id]")?.dataset.id ?? null;

      // U31: Enter on a row selects it. The library would lift on Enter too, so Enter stops here and selects, and only Space lifts.
      if (key.key !== "Enter" || key.metaKey || key.ctrlKey || key.altKey || key.shiftKey || !(key.target instanceof HTMLElement) || !key.target.matches(ROW)) return;

      key.stopPropagation();
      key.preventDefault();

      const id = key.target.closest<HTMLElement>("[data-id]")?.dataset.id;

      if (id === undefined) return;

      rail.select(id);
    };

    window.addEventListener("pointerdown", press, { capture: true });
    window.addEventListener("pointermove", follow, { capture: true });
    container()?.addEventListener("keydown", lift, { capture: true });
    onCleanup(() => {
      window.removeEventListener("pointerdown", press, { capture: true });
      window.removeEventListener("pointermove", follow, { capture: true });
      container()?.removeEventListener("keydown", lift, { capture: true });
    });
  });

  /** Where the drop line sits: at the top of the room the library opened for the dragged row, once it has moved there. */
  const placeDrop = (next: { depth: number } | null) => {
    if (next === null) {
      const back = liftedRow;

      liftedRow = null;
      setDrop(null);

      if (back !== null) {
        const focus = captureFocus();
        requestAnimationFrame(() => focus(() => focusRow(back)));
      }

      return;
    }

    queueMicrotask(() => {
      const base = container()?.getBoundingClientRect();
      const room = container()?.querySelector("[data-dnd-placeholder]")?.getBoundingClientRect();

      setDrop(base && room ? { depth: next.depth, top: room.top - base.top } : null);
    });
  };

  /** The index among all of `parent`'s other children for one counted among the shown ones: a done row folded into its `✓ n done` line is still a sibling. */
  const trueIndex = (id: string, parent: string | null, shown: number): number => {
    if (shown === 0) return 0;

    const anchor = treeRows().filter((row) => row.parent === parent && row.id !== id)[shown - 1];

    return anchor === undefined ? shown : siblingsOf(rail.nodes, parent).filter((node) => node.id !== id).findIndex((node) => node.id === anchor.id) + 1;
  };

  const moveTo = ({ id, parent, index }: SortableMove) => {
    if (daemonExit() !== null || pointerOutside()) return;

    void attempt(() => app.rpc("rail.move", { id, parent, index: trueIndex(id, parent, index) }), id);
  };

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

  const spawnAgentWith = (prompt: string | null, under: string | null = parent()) =>
    guarded(async () => {
      const focus = captureFocus(true);
      const params: SpawnParams = { cwd: project.path, prompt, parent: under, model: null, effort: null };

      const spawned = await app.rpc("agent.spawn", params);

      setWanted({ id: spawned.id, focus });

      if (composing()) endCompose();
    });

  const spawnAgent = () => spawnAgentWith(null);

  /** U427, U428: a Workstream's folder is its Door's, an Agent's is its Worktree's, else the Project's. */
  const directoryOf = (node: RailNode) => node.worktree?.path ?? project.path;

  const rowIcons = (node: RailNode): Pick<RowActions, "folder" | "editor"> => ({
    folder: () => void attempt(() => app.openTarget({ kind: "path", path: directoryOf(node), line: null })),
    editor: () => void attempt(() => app.openInEditor(directoryOf(node))),
  });

  const actionsOf = (node: RailNode): RowActions => {
    const base = rowIcons(node);

    if (node.kind === "workstream") {
      const actions: Extract<RowActions, { kind: "workstream" }> = {
        ...base,
        kind: "workstream",
        add: {
          disabled: !canSpawn(),
          shortcut: parent() === node.id && !composing(),
          run: () => {
            if (!canSpawn()) return;

            if (composing()) field()?.focus();
            else spawnAgentWith(null, node.id);
          },
        },
      };

      if (menu.isClearable(node)) actions.clear = () => menu.requestClear(node);

      return actions;
    }

    const exit = rail.exitOf(node);
    const actions: Extract<RowActions, { kind: "agent" }> = { ...base, kind: "agent" };

    if (menu.isRunning(node, exit)) actions.stop = () => stopFromIcon(node, exit);
    else if (menu.isResumable(node, exit)) actions.resume = () => menu.requestResume(node);

    return actions;
  };

  const stopFromIcon = (node: RailNode, exit: ExitState | null) => {
    menu.requestStop(node, exit);
  };

  /** O11: whether a note has been in flight for 500 ms, when U81's `asking the Door…` shows. */
  const [asking, setAsking] = createSignal(false);

  const submitPrompt = (prompt: string | null) => {
    const snapshot = task() ?? keepDraft(draft());

    if (snapshot.status === "pending" || snapshot.status === "unknown") return;
    const originalParent = taskParent();
    const action = taskAction();

    if (originalParent !== null && !rail.nodes.some((node) => node.id === originalParent && node.kind === "workstream")) {
      failure.show(copy.railDraftUnavailable);

      return;
    }

    const revision = retained.submit(snapshot.revision);

    if (!revision) return;
    const original = rail.nodes.find((node) => node.id === originalParent);
    const door = prompt !== null && action === "note" ? snapshot.destination.recipient ?? original?.id ?? null : null;
    const focus = captureFocus(true);
    const slow = setTimeout(() => setAsking(true), 500);
    guarded(async () => {
      let spawned: string | null = null;

      try {
        if (door !== null) await app.rpc("message.send", { to: door, kind: "note", body: prompt!, replyTo: null });
        else {
          spawned = (await app.rpc("agent.spawn", { cwd: project.path, prompt, parent: originalParent, model: null, effort: null })).id;
        }

        const latest = task()?.revision === revision.revision;
        retained.settle(revision.revision, "acknowledged");

        if (latest) {
          if (spawned !== null) setWanted({ id: spawned, focus });
          endCompose();
        }
      } catch (error) {
        retained.settle(revision.revision, error instanceof RpcError && error.code !== -32004 && error.code !== INTERNAL_CODE ? "failed" : "unknown");
        throw error;
      } finally {
        clearTimeout(slow);
        setAsking(false);
      }
    });
  };

  const canSpawn = () => !pending() && !rail.workstreamCreating() && !rail.doorPending(rail.selected() ?? "") && daemonExit() === null;

  onMount(() => {
    const clear = () => {
      failure.clear();
      rail.clearWorkstreamFailure();
      rail.clearStorageFailure();
    };

    document.addEventListener("click", clear, true);
    onCleanup(() => document.removeEventListener("click", clear, true));
  });

  /** U32, U33: `⌘N` adds an Agent or returns to the open field; `⇧⌘N` opens the field. A kept key acts once. U31, U56, U358: the row menu and `⌘⌫` act on a focused row only; the keys of a control inside a row are its own. */
  registerKeys({ kind: "app" }, [
    { action: CHORDS.find((row) => row.keys === "⌘N")!, match: "key", capture: () => ({ available: () => composing() || canSpawn(), invoke: () => {
      if (composing()) field()?.focus();
      else spawnAgent();
    } }) },
    { action: CHORDS.find((row) => row.keys === "⇧⌘N")!, match: "key", capture: () => ({ available: canSpawn, invoke: openCompose }) },
    { chords: ["ContextMenu", "Shift+F10"], when: (press) => focusedRowId(press) !== undefined, run: openMenuAtFocusedRow },
    { chords: ["ContextMenu", "Shift+F10"], when: (press) => press.target instanceof HTMLElement && press.target.matches("[data-todo-root]"), run: openOverviewMenu },
    { action: CHORDS.find((row) => row.keys === "⌘⌫")!, when: (press) => focusedRowId(press) !== undefined, capture: (focus) => ({ available: () => focus instanceof HTMLElement && focus.isConnected && focus.matches(ROW) && rail.nodes.some((node) => node.id === rail.selected()) && daemonExit() === null, invoke: () => removeSelected(false) }) },
  ]);

  /** Restoring must not scroll before the window has placed focus (U100). */
  createEffect(
    on(rail.selected, (id) => {
      if (id === null || rail.restored()) return;

      queueMicrotask(() => rowElement(id)?.scrollIntoView({ block: "nearest" }));
    }),
  );

  createEffect(() => {
    const request = wanted();

    if (request !== null && rail.nodes.some((node) => node.id === request.id)) {
      request.focus(() => rail.select(request.id));
      setWanted(null);
    }
  });

  return (
    <>
    <button
      type="button"
      data-button="quiet"
      data-todo-root
      aria-pressed={rail.selected() === null}
      aria-label={copy.overviewLabel(attention())}
      data-scrolled={scrolled()}
      class="todo-project-root"
      onClick={() => rail.select(null)}
      onContextMenu={(press: MouseEvent) => openOnRelease(press, (x, y) => {
        const entries = recoveryEntries();

        if (entries.length > 0) void contextMenu(app, entries, x, y);
      })}
    >
      <span class="overview-tile"><Icon name="layout-dashboard" /></span>
      <span>{copy.overview}</span>
      <Show when={attention() > 0}>
        <span class="ink-count" data-state="" data-tone="amber" aria-hidden="true">
          <KindGlyph kind="needs-you" decorative /> {attention()}
        </span>
      </Show>
    </button>
    <div class="rail-scroll" onScroll={(event) => setScrolled(event.currentTarget.scrollTop > 0)}>
    <TreeView.Root
      class="rail-tree"
      ref={(element) => {
        setContainer(element);
        let contextClick = false;
        element.addEventListener("pointerdown", (event) => {
          contextClick = event.button === 0 && event.ctrlKey;

          // macOS Ctrl-click must not focus a row, blur unfinished input or arm its sort sensor.
          if (contextClick) {
            event.preventDefault();
            event.stopPropagation();
          }
        }, true);
        // A native menu may consume the compatibility click; keyboard activation must still work next.
        element.addEventListener("keydown", () => { contextClick = false; }, true);
        element.addEventListener("click", (event) => {
          if (!contextClick && !event.ctrlKey) return;

          contextClick = false;
          event.preventDefault();
          event.stopPropagation();
        }, true);
      }}
      collection={collection()}
      expandedValue={expanded()}
      selectedValue={rail.selected() === null ? [] : [rail.selected()!]}
      expandOnClick={false}
      onSelectionChange={(details) => {
        const id = details.selectedValue.at(-1);

        if (id !== undefined) rail.select(id);
      }}
      onExpandedChange={(details) => {
        const open = new Set(details.expandedValue);

        for (const id of collection().getBranchValues()) if (open.has(id) === collapsed().has(id)) rail.toggleCollapsed(id);
      }}
    >
      <SectionHead icon="messages" label={copy.workstreamsHead} />
      <Show when={composing()}>
        <ReservedLine status={task() && taskParent() !== parent() ? (copy.railDraftTaskTarget(taskParent() === null ? project.name : destinationName(taskParent()!))) : null} />
        <SpawnPromptField
          ref={setField}
          value={draft()}
          selection={task()?.selection ?? null}
          onSelection={(selection) => { if (task()?.status === "draft") keepDraft(draft(), selection); }}
          onInput={keepDraft}
          onSubmit={(prompt) => {
            if (canSpawn()) submitPrompt(prompt);
          }}
          onCancel={() => setComposing(false)}
        />
      </Show>
      <Sortable.Root
        class="rail-sortable"
        ids={nodeIds()}
        tree={{
          rows: treeRows(),
          indent: railStep(),
          nests: (id) => rows().some((row) => row.kind === "node" && row.node.id === id && isWorkstream(row.node) && row.collapsed === null),
          onMove: moveTo,
          onPreview: (next) => placeDrop(next),
        }}
      >
      <TreeView.Tree aria-label={copy.rail} aria-disabled={daemonExit() !== null ? true : undefined}>
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
                        <TreeView.NodeProvider node={entryOf(view().node.id).node} indexPath={entryOf(view().node.id).indexPath}>
                        <Sortable.Item id={view().node.id} index={indexOf().get(view().node.id) ?? 0} disabled={daemonExit() !== null}>{(parts) =>
                        <RailRowView
                          row={view()}
                          exit={rail.exitOf(view().node)}
                          needsYou={isWorkstream(view().node) ? agentKindsBelow(rail.nodes, view().node.id).filter((kind) => kind === "needs-you").length : 0}
                          selected={rail.selected() === view().node.id}
                          tabbable={tabbableId() === view().node.id}
                          branch={collection().isBranchNode(entryOf(view().node.id).node)}
                          onToggle={() => rail.toggleCollapsed(view().node.id)}
                          onFailure={(message) => failure.show(message, view().node.id)}
                          onEditing={(editing) => setEditingName(editing ? view().node.id : null)}
                          onRename={async (name) => {
                            await app.rpc("rail.rename", { id: view().node.id, name });
                            failure.acknowledge(view().node.id);
                          }}
                          onStartDoor={() => { rail.select(view().node.id); void rail.startDoor(view().node.id); }}
                          doorPending={rail.doorPending(view().node.id)}
                          doorFailure={rail.doorFailure(view().node.id)}
                          lifted={parts.isDragging()}
                          sortRef={parts.ref}
                          actions={actionsOf(view().node)}
                          onOpenMenu={(x, y) => void menu.open(view().node, rail.exitOf(view().node), x, y)}
                        />
                        }</Sortable.Item>
                        </TreeView.NodeProvider>
                      )}
                    </Show>
                    <Show when={fold()}>
                      {(view) => (
                        <div class="rail-fold" role="none" style={{ "--indent": `calc(${view().depth} * var(--rail-step) + var(--rail-inset))` }}>
                          <Trunks depth={view().depth} trail={[...view().trail, false]} stem={false} />
                          <button data-button="quiet"
                            class="word"
                            role="treeitem"
                            aria-level={view().depth + 1}
                            onClick={() => setUnfolded((open) => toggled(open, view().parent))}
                          >
                            <KindGlyph kind="done" decorative /> {view().count}{copy.doneCount}</button>
                        </div>
                      )}
                    </Show>
                  </>
                );
              }}
            </Show>
          )}
        </For>
      </TreeView.Tree>
      <Show when={drop()}>
        {(line) => <div class="drop-line" aria-hidden="true" style={{ top: `${line().top}px`, "--depth": line().depth }} />}
      </Show>
      </Sortable.Root>
    </TreeView.Root>
    </div>

      <Show when={recovery().length > 0}>
        <details class="rail-recovery">
          <summary><Icon name="right" />{copy.railDrafts}</summary>
          <For each={recovery().map((entry) => entry.revision).reverse()}>{(revision) => (
            <Show when={revisions().get(revision)}>{(entry) => {
              const text = () => {
                const value = entry().value.text;

                return v.is(v.string(), value) ? value : "";
              };

              const base = () => {
                const value = entry().value.base;

                return v.is(v.string(), value) ? value : null;
              };

              return (
                <details>
                  <summary><Icon name="right" /><span>{copy.railDraftRecovery(entry().destination.surface, destinationName(entry().destination.object), entry().status)}</span></summary>
                  <ReservedLine status={entry().destination.object !== "" && entry().destination.object !== project.path && !rail.nodes.some((node) => node.id === entry().destination.object) ? copy.railDraftUnavailable : null} />
                  <textarea aria-label={copy.railDraftRecovered} readOnly value={text()} />
                  <p>{copy.railDraftDestination(project.name, destinationName(entry().destination.recipient ?? entry().destination.object))}</p>
                  <Show when={entry().destination.surface === "rail-task"}><p>{copy.railDraftTaskAction(entry().value.action === "note" || (entry().value.action === undefined && entry().destination.recipient !== null))}</p></Show>
                  <Show when={entry().destination.surface === "rail-clear"}><p>{entry().value.withoutHandoff === true ? copy.clearWithoutHandoff : copy.clearWithHandoff}</p></Show>
                  <ReservedLine status={entry().destination.surface === "rail-name" && rail.nodes.some((node) => node.id === entry().destination.object && node.name !== entry().value.base) ? copy.railDraftConflict : null} />
                  <Show when={base()}>{(base) => <p>{copy.railDraftBase(base())}</p>}</Show>
                  <div class="rail-recovery-actions">
                  <button data-button="quiet" onClick={() => { void attempt(() => writeText(connected.app, text())); }}>{copy.railDraftCopy}</button>
                  <button data-button="quiet" onClick={() => {
                    const current = task()?.revision === entry().revision;
                    retained.discard(entry().revision);

                    if (current && !task()) setTaskDestination(parent());
                  }}>{copy.railDraftDiscard}</button>
                  </div>
                </details>
              );
            }}</Show>
          )}</For>
        </details>
      </Show>
      <RailConfirm menu={menu} rowElement={rowElement} />
      <Show when={clearPrompt() !== null}>
        <Portal>
          <Show when={clearPrompt()}>
            {(node) => (
              <div
                ref={(element) => {
                  clearElement = element;
                  const observer = new ResizeObserver(placeClear);

                  observer.observe(element);
                  queueMicrotask(placeClear);
                  onCleanup(() => { observer.disconnect(); clearElement = undefined; });
                }}
                class="rail-menu rail-clear" role="dialog" aria-label={copy.clearName(node().name)}
              >
                <ClearPrompt node={node()} onHide={menu.hideClear} />
              </div>
            )}
          </Show>
        </Portal>
      </Show>
      <ReservedLine overlay message={failure.message() ?? rail.workstreamFailure() ?? rail.doorFailure(rail.selected() ?? "") ?? rail.storageFailure() ?? (retained.problem()?.destination?.surface.startsWith("rail-") ? copy.railDraftRetention(project.name, retained.problem()!.destination?.surface ?? "rail", retained.problem()!.operation) : null)}
        status={asking() ? copy.askingTheDoor : retained.records().some(entry => entry.destination.surface.startsWith("rail-") && entry.status === "unknown") ? copy.railDraftUnknown(project.name) : null}>
        <Show when={retained.problem()?.destination?.surface.startsWith("rail-")}><button data-button="quiet" onClick={() => retained.retry()}>{copy.railDraftRetry}</button></Show>
      </ReservedLine>
      <div class="rail-actions">
        <button data-button="add"
          aria-label={copy.addWorkstream}
          class="word"
          aria-disabled={canSpawn() ? undefined : true}
          onClick={() => {
            if (canSpawn()) guarded(() => rail.createWorkstream());
          }}
        >
          <Icon name="plus" />{copy.addWorkstreamWord}</button>
      </div>
    </>
  );
};
