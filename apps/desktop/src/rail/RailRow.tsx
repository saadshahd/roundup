import { copy } from "../copy";
import { createEffect, createSignal, For, onCleanup, Show, type JSX } from "solid-js";
import { Portal } from "solid-js/web";
import { IconButton } from "../ui/IconButton";
import { Tooltip } from "../ui/Tooltip";
import { TreeView } from "../ui/TreeView";
import { ownKeys } from "./rows";
import { Tip } from "./Tip";
import type { TriggerProps } from "./Tip";
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
import { RowIcons } from "./RowIcons";
import type { RowActions } from "./RowIcons";

/** What Ark's item and branch control take for the row's own element. */
type RowProps = JSX.HTMLAttributes<HTMLDivElement>;

/** U403: a folded Workstream shows at most this many Agent Glyphs before its count. */
const FOLDED_GLYPHS = 5;

const NameField = (props: { node: RailNode; onCommit: (name: string) => void; onCancel: () => void }) => (
  <input
    aria-label={copy.name}
    value={props.node.name}
    ref={(field) =>
      queueMicrotask(() => {
        field.focus();
        field.select();
      })
    }
    onKeyDown={(key) => {
      if (key.key === "Enter") {
        const name = key.currentTarget.value.trim();

        if (name === "") props.onCancel();
        else props.onCommit(name);
      }

      if (key.key === "Escape") props.onCancel();
    }}
    onBlur={props.onCancel}
  />
);


/** U2228a: a hover on a button or input of the row belongs to that control's own tooltip, never the row's. */
const ignoringControls = <T extends { onPointerMove?: unknown; onPointerLeave?: unknown }>(props: T): T => {
  // SAFETY: the row's Ark tooltip trigger props carry these as pointer-event handlers when present.
  const move = props.onPointerMove as ((event: PointerEvent) => void) | undefined;
  // SAFETY: same trigger props; onPointerLeave is the handler that closes the row's tooltip.
  const leave = props.onPointerLeave as ((event: PointerEvent) => void) | undefined;

  return {
    ...props,
    onPointerMove: (event: PointerEvent) => {
      if (!(event.target instanceof Element) || !event.target.closest("button, input")) move?.(event);
      else leave?.(event);
    },
  };
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
  onRename: (name: string) => void;
  onStartDoor: () => void;
  doorPending: boolean;
  doorFailure: string | null;
  /** U164: how many of this Agent's Runs are running; shown as `· n` after the name, and hidden at zero. */
  runs?: number;
  lifted: boolean;
  sortRef: (element: HTMLElement | undefined) => void;
  onOpenMenu: (press: MouseEvent) => void;
  /** U426: the icons of a Workstream or Agent row. */
  actions: RowActions;
}) => {
  const { near, handlers } = createNear(() => props.selected);
  const [editing, setEditing] = createSignal(false);
  const pads = createAgentPads(() => props.row.node);

  const workstream = () => isWorkstream(props.row.node);

  // U2228a: a name cut to `…` in the layout it has now (hovered, focused or selected included) shows in full in the row's Tooltip.
  const [cut, setCut] = createSignal(false);
  let nameBox: HTMLSpanElement | undefined;
  const measure = () => setCut(!!nameBox && nameBox.scrollWidth > nameBox.clientWidth);

  createEffect(() => {
    props.row.node.name;
    props.runs;
    near();
    props.selected;
    editing();
    queueMicrotask(measure);
  });

  createEffect(() => {
    const row = nameBox?.closest(".rail-row");

    if (!nameBox || !row || typeof ResizeObserver === "undefined") return;

    const observer = new ResizeObserver(measure);
    observer.observe(row);
    observer.observe(nameBox);
    onCleanup(() => observer.disconnect());
  });

  const kind = (): Kind | null => props.row.node.status?.kind ?? null;

  const inkTone = () => {
    const current = kind();

    return current !== null && hasInk(glyphOf(current)) ? glyphOf(current).tone : undefined;
  };

  /** U172, U3704: the Door action shows when the row is near; it takes no room at rest, so a name keeps the row's free room. */
  const hasAction = () => workstream() && props.exit !== null;

  const showsAction = () => near() && hasAction();

  const needsYouShown = () => workstream() && !props.row.collapsed && props.needsYou > 0;

  /** U521: what the row's own Tooltip says: the name a cut hides, the status label, and the counts at the row's right. The same text the row shows, unchanged. */
  const lines = (): string[] => [
    ...(cut() ? [props.row.node.name] : []),
    ...(workstream() || !props.row.node.status ? [] : [props.row.node.status.label]),
    ...(needsYouShown() ? [copy.needYou(props.needsYou)] : []),
    ...(workstream() && props.row.collapsed ? [copy.agents(props.row.collapsed.agents.length)] : []),
  ];

  // The Tooltip anchors to the row itself: the tree item's props replace the id its trigger lookup needs.
  let rowBox: HTMLElement | undefined;

  const nameId = () => `rail-name-${props.row.node.id}`;

  const rowProps = () => ({
    class: "rail-row",
    "data-id": props.row.node.id,
    // The Rail's selected mark and the drop target both read the row itself.
    "data-selected": props.selected ? "true" : undefined,
    tabIndex: props.tabbable ? 0 : -1,
    "data-near": near(),
    "data-kind": kind() ?? undefined,
    "data-lifted": props.lifted,
    style: { "--indent": `calc(${props.row.depth} * var(--rail-step) + var(--rail-inset))` },
    onContextMenu: (press: MouseEvent) => {
      press.preventDefault();
      props.onOpenMenu(press);
    },
    ...handlers,
  });

  /** The row's own element: Ark's item or branch control, which the Tooltip's trigger props join. */
  const row = (mine: (extra: RowProps) => RowProps) => (
    <Tooltip.Trigger
      asChild={(trigger) => (
        // SAFETY: the Tooltip types its trigger for a button; the row is a div, and the props are the same event and ARIA attributes.
        <div {...ignoringControls(trigger(mine(rowProps()) as TriggerProps))} ref={(el: HTMLDivElement) => { rowBox = el; props.sortRef(el); }}>
          <Trunks depth={props.row.depth} trail={props.row.trail} stem={props.row.children || pads.shown()} />
          <p class="line">
            <Show when={workstream()} fallback={<KindGlyph kind={kind() ?? "idle"} />}>
              <IconButton
                class="rail-chevron"
                aria-label={copy.collapse}
                {...ownKeys}
                onClick={(click) => {
                  click.stopPropagation();
                  props.onToggle();
                }}
              >
                <Icon name={props.row.collapsed ? "right" : "down"} />
              </IconButton>
            </Show>
            <Show
              when={editing()}
              fallback={
                <span
                  id={nameId()}
                  class="name"
                  ref={nameBox}
                  classList={{ ink: inkTone() !== undefined }}
                  data-tone={inkTone()}
                  onDblClick={() => setEditing(true)}
                >
                  {props.row.node.name}
                  <Show when={(props.runs ?? 0) > 0}>
                    <span class="runs">{copy.separator}{props.runs}</span>
                  </Show>
                </span>
              }
            >
              <NameField
                node={props.row.node}
                onCommit={(name) => {
                  props.onRename(name);
                  setEditing(false);
                }}
                onCancel={() => setEditing(false)}
              />
            </Show>
            <Show when={workstream() ? (props.doorFailure !== null ? "error" : kind()) : null}>{(current) => <KindGlyph kind={current()} />}</Show>
            <Show when={workstream()}>
              <Show
                when={props.row.collapsed}
                fallback={
                  <Show when={props.needsYou > 0}>
                    <span class="ink-count" data-tone="amber" data-state="needs-you">
                      <KindGlyph kind="needs-you" decorative /> {props.needsYou}
                    </span>
                  </Show>
                }
              >
                {(collapsed) => (
                  <span class="folded-agents" data-state="folded">
                    <For each={collapsed().agents.slice(0, FOLDED_GLYPHS)}>{(each) => <KindGlyph kind={each} />}</For>
                    <span class="light">{collapsed().agents.length}</span>
                  </span>
                )}
              </Show>
            </Show>
            <span class="fill" aria-hidden="true" />
            <span class="row-tail">
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
              <PadsControl pads={pads} />
            </span>
          </p>
        </div>
      )}
    />
  );

  return (
    <>
      <Tooltip.Root disabled={lines().length === 0} closeOnEscape={false} positioning={{ strategy: "fixed", placement: "bottom-start", getAnchorRect: () => rowBox?.getBoundingClientRect() ?? null }}>
        <Show
          when={props.branch}
          fallback={
            <TreeView.Item data-id={props.row.node.id} aria-labelledby={nameId()} asChild={(item) => row((extra) => item(extra))} />
          }
        >
          <TreeView.Branch
            data-id={props.row.node.id}
            aria-labelledby={nameId()}
            // A click on the treeitem itself, not on a part of it (a script's or a test's `[data-id]` query lands here), is the click of its control.
            onClick={(click) => {
              if (click.target === click.currentTarget) click.currentTarget.querySelector<HTMLElement>('[data-part="branch-control"]')?.click();
            }}
          >
            <TreeView.BranchControl asChild={(control) => row((extra) => control(extra))} />
          </TreeView.Branch>
        </Show>
        <Show when={lines().length > 0}>
          <Portal>
            <Tooltip.Positioner>
              <Tooltip.Content class="rail-name-tip">
                <For each={lines()}>{(line) => <div>{line}</div>}</For>
              </Tooltip.Content>
            </Tooltip.Positioner>
          </Portal>
        </Show>
      </Tooltip.Root>
      <PadRows pads={pads} depth={props.row.depth} trail={props.row.trail} children={props.row.children} />
    </>
  );
};
