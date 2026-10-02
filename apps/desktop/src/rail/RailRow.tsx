import { createSignal, Show } from "solid-js";
import type { Accessor } from "solid-js";
import type { RailNode } from "@contracts/agent/RailNode";
import type { Kind } from "@contracts/Kind";
import { glyphOf, hasInk } from "../ink/glyph";
import { KindGlyph } from "../ink/KindGlyph";
import type { ExitState } from "../state/rail";
import { isPlainGroup } from "./layout";
import type { NodeRow } from "./layout";
import { isUnprompted, liveLineOf } from "./liveLine";

/** A Kind's mark, or a bare mark for a plain Group, which has no Kind. */
type Mark = { kind: Kind } | { bare: string };

/** A Terminal has no Kind, so it borrows the marks of `working` and `done`. */
const markOfRow = (row: NodeRow, exit: ExitState | null): Mark => {
  if (row.collapsed) return row.collapsed.kind ? { kind: row.collapsed.kind } : { bare: "▸" };

  if (row.node.status) return { kind: row.node.status.kind };

  if (row.node.kind === "terminal") return { kind: exit ? "done" : "working" };

  return { bare: "▾" };
};

const MarkView = (props: { mark: Mark }) => (
  <Show when={"kind" in props.mark ? props.mark.kind : null} fallback={<span class="glyph" data-tone="light">{"bare" in props.mark ? props.mark.bare : ""}</span>}>
    {(kind) => <KindGlyph kind={kind()} />}
  </Show>
);

const NameField = (props: { node: RailNode; onCommit: (name: string) => void; onCancel: () => void }) => (
  <input
    aria-label="name"
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

export const RailRowView = (props: {
  row: NodeRow;
  exit: ExitState | null;
  selected: boolean;
  /** Roving tabindex: exactly one row in the Rail is reachable by Tab. */
  tabbable: boolean;
  now: Accessor<number>;
  onSelect: () => void;
  onToggle: () => void;
  onRename: (name: string) => void;
  onPromote: () => void;
  /** A drag is under way: live lines are hidden so the rows keep the heights the drag measured. */
  dragging: boolean;
  lifted: boolean;
  /** `1` or `-1` while a drag moves the row into the room it opened, `0` otherwise. */
  shift: number;
  onPointerDown: (press: PointerEvent) => void;
}) => {
  const [hovered, setHovered] = createSignal(false);
  const [editing, setEditing] = createSignal(false);

  const mark = () => markOfRow(props.row, props.exit);

  const kind = () => {
    const current = mark();

    return "kind" in current ? current.kind : null;
  };

  const inkTone = () => {
    const current = kind();

    return current !== null && hasInk(glyphOf(current)) ? glyphOf(current).tone : undefined;
  };

  const liveLine = () => liveLineOf(props.row.node, props.exit, props.now());
  const showsLiveLine = () => !props.dragging && (props.selected || hovered() || isUnprompted(props.row.node));

  return (
    <div
      class="rail-row"
      role="treeitem"
      aria-level={props.row.depth + 1}
      aria-selected={props.selected}
      aria-expanded={isPlainGroup(props.row.node) ? props.row.collapsed === null : undefined}
      data-selected={props.selected}
      data-id={props.row.node.id}
      data-lifted={props.lifted}
      data-shift={props.shift}
      tabIndex={props.tabbable ? 0 : -1}
      style={{ "padding-left": `${props.row.depth * 2}ch` }}
      onClick={props.onSelect}
      onPointerDown={(press) => {
        if (!(press.target instanceof Element) || !press.target.closest("button, input")) props.onPointerDown(press);
      }}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <p class="line">
        <Show
          when={isPlainGroup(props.row.node)}
          fallback={<MarkView mark={mark()} />}
        >
          <button
            class="word"
            aria-label="collapse"
            onClick={(click) => {
              click.stopPropagation();
              props.onToggle();
            }}
          >
            <MarkView mark={mark()} />
          </button>
        </Show>
        <Show
          when={editing()}
          fallback={
            <span
              class="name"
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
            onCommit={(name) => {
              props.onRename(name);
              setEditing(false);
            }}
            onCancel={() => setEditing(false)}
          />
        </Show>
        <Show when={props.row.collapsed}>
          {(collapsed) => <span class="light">{collapsed().children}</span>}
        </Show>
        <Show when={hovered() && isPlainGroup(props.row.node)}>
          <button
            class="word"
            onClick={(click) => {
              click.stopPropagation();
              props.onPromote();
            }}
          >
            promote
          </button>
        </Show>
      </p>
      <Show when={showsLiveLine() ? liveLine() : null}>
        {(text) => <p class="live light">{text()}</p>}
      </Show>
    </div>
  );
};
