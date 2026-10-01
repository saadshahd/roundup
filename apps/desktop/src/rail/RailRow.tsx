import { createSignal, Show } from "solid-js";
import type { Accessor } from "solid-js";
import type { RailNode } from "@contracts/agent/RailNode";
import { glyphOf, hasInk } from "../ink/glyph";
import type { Glyph } from "../ink/glyph";
import type { ExitState } from "../state/rail";
import { isPlainGroup } from "./layout";
import type { RailRow } from "./layout";
import { isUnprompted, liveLineOf } from "./liveLine";

type NodeRow = Extract<RailRow, { kind: "node" }>;

const COLLAPSED_WITHOUT_AGENT: Glyph = { mark: "▸", tone: "light" };

const PLAIN_GROUP: Glyph = { mark: "▾", tone: "light" };

/** A Terminal has no Kind, so it borrows the marks of `working` and `done`. */
const glyphOfRow = (row: NodeRow, exit: ExitState | null): Glyph => {
  if (row.collapsed) return row.collapsed.kind ? glyphOf(row.collapsed.kind) : COLLAPSED_WITHOUT_AGENT;

  if (row.node.status) return glyphOf(row.node.status.kind);

  if (row.node.kind === "terminal") return glyphOf(exit ? "done" : "working");

  return PLAIN_GROUP;
};

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
  now: Accessor<number>;
  onSelect: () => void;
  onToggle: () => void;
  onRename: (name: string) => void;
  onPromote: () => void;
}) => {
  const [hovered, setHovered] = createSignal(false);
  const [editing, setEditing] = createSignal(false);

  const glyph = () => glyphOfRow(props.row, props.exit);
  const ink = () => hasInk(glyph());
  const liveLine = () => liveLineOf(props.row.node, props.exit, props.now());
  const showsLiveLine = () => props.selected || hovered() || isUnprompted(props.row.node);

  return (
    <div
      class="rail-row"
      role="treeitem"
      aria-level={props.row.depth + 1}
      aria-selected={props.selected}
      aria-expanded={isPlainGroup(props.row.node) ? props.row.collapsed === null : undefined}
      data-selected={props.selected}
      style={{ "padding-left": `${props.row.depth * 2}ch` }}
      onClick={props.onSelect}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <p class="line">
        <Show
          when={isPlainGroup(props.row.node)}
          fallback={
            <span class="glyph" classList={{ ink: ink() }} data-tone={glyph().tone}>
              {glyph().mark}
            </span>
          }
        >
          <button
            class="word glyph"
            aria-label="fold"
            data-tone={glyph().tone}
            classList={{ ink: ink() }}
            onClick={(click) => {
              click.stopPropagation();
              props.onToggle();
            }}
          >
            {glyph().mark}
          </button>
        </Show>
        <Show
          when={editing()}
          fallback={
            <span
              class="name"
              classList={{ ink: ink() }}
              data-tone={ink() ? glyph().tone : undefined}
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
