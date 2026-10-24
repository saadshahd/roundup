import { captureFocus, queueFocus } from "../keys/focus";
import { ReservedLine } from "../ink/ReservedLine";
import { openOnRelease } from "../ui/contextMenu";
import * as v from "valibot";
import { useConnectedProject } from "../state/connectedProject";
import "../keys/styles.css";
import { copy } from "../copy";
import { createEffect, createSignal, For, onCleanup, Show, type JSX } from "solid-js";
import { IconButton } from "../ui/IconButton";
import { TreeView } from "../ui/TreeView";
import { ownKeys } from "./rows";
import { Tip } from "../ui/Tip";
import { createCutTitle } from "../ui/cutTitle";
import type { RailNode } from "@contracts/agent/RailNode";
import type { Kind } from "@contracts/Kind";
import { glyphOf, hasInk } from "../ink/glyph";
import { Icon } from "../ink/Icon";
import { KindGlyph } from "../ink/KindGlyph";
import type { ExitState } from "../state/rail";
import { isWorkstream } from "./layout";
import { Trunks } from "./Trunks";
import type { NodeRow } from "./layout";
import { createAgentPads, PadRows, PadsControl } from "./pads/AgentPads";
import { createNear } from "../ink/near";
import { INTERNAL_CODE, RpcError } from "../app/seam";
import { draftsOf } from "./persist/storage";
import { RowIcons } from "./RowIcons";
import type { RowActions } from "./RowIcons";

/** What Ark's item and branch control take for the row's own element. */
type RowProps = JSX.HTMLAttributes<HTMLDivElement>;

/** U403: a folded Workstream shows at most this many Agent Glyphs before its count. */
const FOLDED_GLYPHS = 5;

const NameField = (props: { node: RailNode; onCommit: (name: string) => Promise<void>; onFailure: (message: string) => void; onCancel: () => void }) => {
  const connected = useConnectedProject();
  const drafts = draftsOf(connected.drafts);
  const destination = { surface: "rail-name", object: props.node.id, recipient: null };
  const current = () => drafts.records().filter((entry) => entry.destination.surface === destination.surface && entry.destination.object === destination.object).at(-1);

  const value = () => {
    const text = current()?.value.text;

    return v.is(v.string(), text) ? text : props.node.name;
  };

  const edit = (field: HTMLInputElement) => drafts.edit(current()?.destination ?? destination,
    { text: field.value, base: current()?.value.base ?? props.node.name },
    { start: field.selectionStart ?? 0, end: field.selectionEnd ?? 0, direction: field.selectionDirection ?? "none" }, "ordinary");

  const conflict = () => current() !== undefined && current()?.status !== "pending" && current()?.value.base !== props.node.name;

  const commit = async (field: HTMLInputElement) => {
    if (conflict()) return;
    const snapshot = current() ?? edit(field);
    const name = value().trim();

    if (name === "") {
      props.onCancel();

      return;
    }

    const submitted = drafts.submit(snapshot.revision);

    if (!submitted) return;

    try {
      await props.onCommit(name);
      const latest = current()?.revision === submitted.revision;
      drafts.settle(submitted.revision, "acknowledged");

      if (latest) props.onCancel();
    } catch (error) {
      drafts.settle(submitted.revision, error instanceof RpcError && error.code !== -32004 && error.code !== INTERNAL_CODE ? "failed" : "unknown");

      if (!(error instanceof Error)) throw error;
      props.onFailure(error.message);
    }
  };

  return <>
    <input
      type="text"
      aria-label={copy.name}
      aria-invalid={conflict() || current()?.status === "failed" ? true : undefined}
      value={value()}
      ref={(field) => {
        captureFocus(true);
        queueFocus(() => {
        field.focus();
        const selection = current()?.selection;

        if (selection) field.setSelectionRange(selection.start, selection.end, selection.direction);
        else field.select();
      }); }}
      onInput={(event) => edit(event.currentTarget)}
      onSelect={(event) => { if (event.currentTarget.isConnected && current()?.status === "draft") edit(event.currentTarget); }}
      onKeyDown={(key) => {
        if (key.key === "Enter") { key.preventDefault(); void commit(key.currentTarget); }

        if (key.key === "Escape") props.onCancel();
      }}
      onBlur={props.onCancel}
    />
    <ReservedLine overlay message={conflict() ? copy.railDraftConflict : null} />
  </>;
};


export const RailRowView = (props: {
  row: NodeRow;
  exit: ExitState | null;
  /** U403: the Workstream's `needs-you` Agents, shown as an Ink count at its right when open. */
  needsYou: number;
  selected: boolean;
  /** U31: the one row Tab reaches, the selected row else the first; Ark would rove it with focus. */
  tabbable: boolean;
  /** Whether Ark's tree walks this row as a branch (a Workstream, or a row with rows of its own below it) or as a leaf. */
  branch: boolean;
  onToggle: () => void;
  onEditing?: (editing: boolean) => void;
  onRename: (name: string) => Promise<void>;
  onFailure: (message: string) => void;
  onStartDoor: () => void;
  doorPending: boolean;
  doorFailure: string | null;
  lifted: boolean;
  sortRef: (element: HTMLElement | undefined) => void;
  onOpenMenu: (x: number, y: number) => void;
  /** U426: the icons of a Workstream or Agent row. */
  actions: RowActions;
}) => {
  const { worktrees, queue, unseen } = useConnectedProject();

  /** U3566: the row's own finish is unread, or a folded Workstream hides an unread one below it. */
  const unread = () => unseen.of(props.row.node.id) || (workstream() && props.row.collapsed !== null && unseen.below(props.row.node.id));

  /** a queued Workstream's place, or the Todo it waits for; gone once its Door starts or the entry leaves. */
  const queuedBadge = () => {
    const queued = workstream() ? queue.entryOf(props.row.node.id) : null;

    if (!queued) return null;

    return queued.entry.start.kind === "afterTodo" ? copy.afterBadge(queued.entry.start.todo) : copy.queuedBadge(queued.place);
  };

  const worktreeFailure = () => worktrees.read(props.row.node.id)?.failure;

  const facts = () => {
    const read = worktrees.read(props.row.node.id);

    return read?.failure === null ? read.state : null;
  };

  const { near, handlers } = createNear(() => props.selected);
  const [editing, setEditing] = createSignal(false);
  createEffect(() => props.onEditing?.(editing()));
  onCleanup(() => { if (editing()) props.onEditing?.(false); });
  const pads = createAgentPads(() => props.row.node);

  const workstream = () => isWorkstream(props.row.node);

  const cutName = createCutTitle(() => props.row.node.name, {
    track: () => { near(); props.selected; editing(); },
    watch: (box) => box.closest(".rail-row"),
  });

  const kind = (): Kind | null => props.row.node.status?.kind ?? null;

  const inkTone = () => {
    const current = kind();

    return current !== null && hasInk(glyphOf(current)) ? glyphOf(current).tone : undefined;
  };

  /** U172: the Door action is whole; the row's states stay beside the name (U3704) and the name gives way. */
  const showsAction = () => workstream() && props.exit !== null;

  const needsYouShown = () => workstream() && !props.row.collapsed && props.needsYou > 0;

  /** U2228a: the row's accessible name: the name, its status label and the counts at its right, which no tooltip repeats (U2228c). */
  const label = () => [
    props.row.node.name,
    ...(workstream() || !props.row.node.status ? [] : [props.row.node.status.label]),
    ...(unread() ? [copy.unreadResult] : []),
    ...(needsYouShown() ? [copy.needYou(props.needsYou)] : []),
    ...(workstream() && props.row.collapsed ? [copy.agents(props.row.collapsed.agents.length)] : []),
  ].join(copy.comma);

  const nameId = () => `rail-name-${props.row.node.id}`;

  const rowProps = () => ({
    class: "rail-row",
    "data-id": props.row.node.id,
    title: !editing() ? cutName.title() : undefined,
    // The Rail's selected mark and the drop target both read the row itself.
    "data-selected": props.selected ? "true" : undefined,
    tabIndex: props.tabbable ? 0 : -1,
    "data-near": near(),
    "data-editing": editing(),
    "data-kind": kind() ?? undefined,
    "data-lifted": props.lifted,
    style: { "--indent": `calc(${props.row.depth} * var(--rail-step) + var(--rail-inset))` },
    onContextMenu: (press: MouseEvent) => openOnRelease(press, (x, y) => props.onOpenMenu(x, y)),
    ...handlers,
  });

  const name = () => (
    <Show
      when={editing()}
      fallback={
        <span
          id={nameId()}
          class="name"
          ref={cutName.ref}
          classList={{ ink: inkTone() !== undefined }}
          data-tone={inkTone()}
          onDblClick={() => setEditing(true)}
        >
          {props.row.node.name}
        </span>
      }
    >
      <NameField
        node={props.row.node}
        onCommit={props.onRename}
        onFailure={props.onFailure}
        onCancel={() => setEditing(false)}
      />
    </Show>
  );

  /** An empty `title` ends the row's native title at a control, which keeps its own tooltip only (U2228a). */
  const untitled = { title: "" };

  /** The row's own element: Ark's item or branch control. */
  const row = (mine: (extra: RowProps) => RowProps) => (
    <div {...mine(rowProps())} ref={(el: HTMLDivElement) => props.sortRef(el)}>
      <Trunks depth={props.row.depth} trail={props.row.trail} stem={props.row.children || pads.shown()} />
      <p class="line">
        <Show when={workstream()} fallback={<KindGlyph kind={kind() ?? "idle"} />}>
          <IconButton
            class="rail-chevron"
            {...untitled}
            aria-label={copy.collapse}
            {...ownKeys}
            onClick={(click) => {
              click.stopPropagation();
              props.onToggle();
            }}
          >
            <Icon name={props.row.collapsed ? "right" : "down"} />
          </IconButton>
          <KindGlyph kind={kind() ?? "idle"} />
        </Show>
        <Show when={workstream() || !props.row.node.worktree} fallback={
          <span class="row-name-state">
            {name()}
            <span class="worktree-chips">
              <Show when={(facts()?.ahead ?? 0) > 0}>
                <span class="worktree-chip" role="img" data-state="ahead" aria-label={copy.worktreeAheadState(facts()!.ahead)}>{copy.worktreeAhead(facts()!.ahead)}</span>
              </Show>
              <Show when={facts()?.ci}>{(ci) => (
                <span class="worktree-chip" role="img" data-state={ci().state} data-stale={ci().stale} aria-label={copy.worktreeCiText(ci().state, ci().stale, true)}><span>{copy.worktreeCiChip(ci().state)}</span><Show when={ci().stale}><span class="worktree-stale">{copy.worktreeStale}</span></Show></span>
              )}</Show>
              <Show when={facts()?.pr}>{(pr) => (
                <span class="worktree-chip" role="img" data-state="pr" data-stale={pr().stale} aria-label={copy.worktreePrState(pr().number, pr().stale)}>{copy.worktreePrText(pr().number, false)}<Show when={pr().stale}><span class="worktree-stale">{copy.worktreeStale}</span></Show></span>
              )}</Show>
              <Show when={worktreeFailure()}>
                <span class="worktree-chip" data-state="unavailable" role="img" aria-label={copy.worktreeUnavailable}>{copy.worktreeUnavailableMark}</span>
              </Show>
            </span>
          </span>
        }>{name()}</Show>
        <Show when={unread()}><span class="unread-dot" data-state="" role="img" aria-label={copy.unreadResult}>{copy.unseenMark}</span></Show>
        <Show when={queuedBadge()}>{(badge) => <span class="light queue-badge">{badge()}</span>}</Show>
        <Show when={workstream()}>
          <Show
            when={props.row.collapsed}
            fallback={
              <Show when={props.needsYou > 0}>
                <span class="ink-count" data-state="" data-tone="amber">
                  <KindGlyph kind="needs-you" decorative /> {props.needsYou}
                </span>
              </Show>
            }
          >
            {(collapsed) => (
              <span class="folded-agents" data-state="">
                <For each={collapsed().agents.slice(0, FOLDED_GLYPHS)}>{(each) => <KindGlyph kind={each} />}</For>
                <span class="light">{collapsed().agents.length}</span>
              </span>
            )}
          </Show>
        </Show>
        <span class="fill" aria-hidden="true" />
        <span class="row-tail" {...untitled}>
          <RowIcons actions={props.actions}>
            <Show when={showsAction()}>
              <Tip text={props.doorFailure ? copy.retryDoor : copy.startDoor}>
                {(tip) => (
                  <button
                    {...tip({
                      ...ownKeys,
                      "aria-disabled": props.doorPending ? true : undefined,
                      onClick: (click: MouseEvent) => {
                        click.stopPropagation();

                        if (!props.doorPending) props.onStartDoor();
                      },
                    })}
                    type="button"
                    data-button="icon"
                    class="door-action"
                    aria-label={props.doorFailure ? copy.retryDoor : copy.startDoor}
                  >
                    <Icon name="play" />
                    <span class="door-action-label">{props.doorFailure ? copy.retryDoor : copy.startDoor}</span>
                  </button>
                )}
              </Tip>
            </Show>
          </RowIcons>
        </span>
        <PadsControl pads={pads} />
      </p>
      <ReservedLine overlay message={pads.failure.message()}>
        <button type="button" data-button="quiet" onClick={pads.failure.clear}>{copy.close}</button>
      </ReservedLine>
    </div>
  );

  return (
    <>
      <Show
        when={props.branch}
        fallback={
          <TreeView.Item data-id={props.row.node.id} aria-label={label()} asChild={(item) => row((extra) => item(extra))} />
        }
      >
        <TreeView.Branch
          data-id={props.row.node.id}
          aria-label={label()}
          // A click on the treeitem itself, not on a part of it (a script's or a test's `[data-id]` query lands here), is the click of its control.
          onClick={(click) => {
            if (click.target === click.currentTarget) click.currentTarget.querySelector<HTMLElement>('[data-part="branch-control"]')?.click();
          }}
        >
          <TreeView.BranchControl asChild={(control) => row((extra) => control(extra))} />
        </TreeView.Branch>
      </Show>
      <PadRows pads={pads} depth={props.row.depth} trail={props.row.trail} children={props.row.children} />
    </>
  );
};
