import { createSignal, onCleanup } from "solid-js";
import type { Accessor } from "solid-js";
import type { RailNode } from "@contracts/agent/RailNode";
import { dropAt, isInPlace, movingWith, remainingRows } from "./dragTarget";
import type { Drop } from "./dragTarget";
import type { NodeRow } from "./layout";

/** A press that moves less than this is a click. */
const THRESHOLD_PX = 4;

export type DragState = {
  drop: Drop;
  /** Where the drop line sits, in pixels from the top of the Rail: at the edge of the room the shifted rows opened. */
  top: number;
  /** The height the dragged rows take: how far a shifted row moves. */
  room: number;
  /** How far the pointer has moved down since the press: the dragged rows follow it. */
  lift: number;
  /** Ids of the dragged row and the rows that move with it. */
  lifted: ReadonlySet<string>;
  /** The rows between the dragged row and the drop line: `1` moves one down into the room opened above it, `-1` one up into the room it left. */
  shifts: ReadonlyMap<string, 1 | -1>;
};

/** Row positions measured when the drag started, so rows shifting during it cannot move the targets. */
type Snapshot = {
  nodes: readonly RailNode[];
  rows: readonly NodeRow[];
  bounds: readonly { top: number; bottom: number }[];
  lifted: ReadonlySet<string>;
  /** The gap the dragged row came from, in the numbering of `rows`. */
  origin: number;
  room: number;
  /** One indent step (2ch) in pixels. */
  step: number;
};

const measured = (container: HTMLElement, nodes: readonly RailNode[], rows: readonly NodeRow[], id: string): Snapshot => {
  const base = container.getBoundingClientRect().top;
  const remaining = remainingRows(nodes, rows, id);
  const moving = movingWith(nodes, id);

  const boundsOf = (rowId: string) => {
    const row = container.querySelector(`[data-id="${CSS.escape(rowId)}"]`);

    if (!row) throw new Error(`no row element for ${rowId}`);

    const { top, bottom } = row.getBoundingClientRect();

    return { top: top - base, bottom: bottom - base };
  };

  const probe = container.appendChild(document.createElement("span"));
  probe.style.display = "inline-block";
  probe.style.width = "2ch";
  const step = probe.getBoundingClientRect().width;
  probe.remove();

  if (step <= 0) throw new Error("the Rail's indent step has no width");

  const lifted = rows.flatMap((row) => (moving.has(row.node.id) ? [row.node.id] : []));
  const liftedBounds = lifted.map(boundsOf);

  return {
    nodes,
    rows: remaining,
    bounds: remaining.map((row) => boundsOf(row.node.id)),
    lifted: new Set(lifted),
    origin: rows.findIndex((row) => row.node.id === id),
    room: Math.max(...liftedBounds.map((bounds) => bounds.bottom)) - Math.min(...liftedBounds.map((bounds) => bounds.top)),
    step,
  };
};

const stateAt = (snapshot: Snapshot, id: string, base: DOMRect, pointer: { x: number; y: number }, lift: number): DragState => {
  const slot = snapshot.bounds.filter((bounds) => (bounds.top + bounds.bottom) / 2 < pointer.y - base.top).length;

  return {
    drop: dropAt(snapshot.nodes, snapshot.rows, id, slot, Math.round((pointer.x - base.left) / snapshot.step)),
    top: (snapshot.bounds[slot - 1]?.bottom ?? 0) - (slot > snapshot.origin ? snapshot.room : 0),
    room: snapshot.room,
    lift,
    lifted: snapshot.lifted,
    shifts: new Map(
      slot < snapshot.origin
        ? snapshot.rows.slice(slot, snapshot.origin).map((row) => [row.node.id, 1] as const)
        : snapshot.rows.slice(snapshot.origin, slot).map((row) => [row.node.id, -1] as const),
    ),
  };
};

export type RailDrag = { state: Accessor<DragState | null>; start: (id: string, press: PointerEvent) => void };

/** The click the browser sends after a release would clear the failure line a rejected `rail.move` has already shown. */
const swallowNextClick = (): (() => void) => {
  const swallow = (click: Event) => click.stopPropagation();
  const forget = () => window.removeEventListener("click", swallow, { capture: true });

  window.addEventListener("click", swallow, { capture: true, once: true });
  setTimeout(forget);

  return forget;
};

/** Pointer-driven drag of a Rail row: releasing calls `onDrop` unless the row lands where it already is; Escape or a cancelled pointer drops nothing. */
export const createRailDrag = (source: {
  container: () => HTMLElement | undefined;
  nodes: () => readonly RailNode[];
  /** Read after `onDragging(id)` has run, so the rows are the ones laid out for the drag. */
  rows: () => readonly NodeRow[];
  /** The id from the first move past a click until release, then `null`; the Rail lays its rows out for the drag in between, before they are measured. */
  onDragging: (id: string | null) => void;
  onDrop: (id: string, drop: Drop) => void;
}): RailDrag => {
  const [state, setState] = createSignal<DragState | null>(null);

  let stop: () => void = () => {};

  let forgetClick: () => void = () => {};

  const start = (id: string, press: PointerEvent): void => {
    const container = source.container();

    if (!container || press.button !== 0) return;

    stop();

    const nodes = [...source.nodes()];
    let snapshot: Snapshot | null = null;

    const follow = (move: PointerEvent) => {
      if (!snapshot && Math.hypot(move.clientX - press.clientX, move.clientY - press.clientY) < THRESHOLD_PX) return;

      if (!snapshot) source.onDragging(id);

      snapshot ??= measured(container, nodes, source.rows(), id);
      setState(
        stateAt(snapshot, id, container.getBoundingClientRect(), { x: move.clientX, y: move.clientY }, move.clientY - press.clientY),
      );
    };

    const release = () => {
      const landed = state();

      stop();

      if (landed) forgetClick = swallowNextClick();

      if (landed && !isInPlace(nodes, id, landed.drop)) source.onDrop(id, landed.drop);
    };

    const cancelOnEscape = (key: KeyboardEvent) => {
      if (key.key === "Escape") stop();
    };

    stop = () => {
      window.removeEventListener("pointermove", follow);
      window.removeEventListener("pointerup", release);
      window.removeEventListener("pointercancel", stop);
      window.removeEventListener("keydown", cancelOnEscape);
      setState(null);
      source.onDragging(null);
    };

    window.addEventListener("pointermove", follow);
    window.addEventListener("pointerup", release);
    window.addEventListener("pointercancel", stop);
    window.addEventListener("keydown", cancelOnEscape);
  };

  onCleanup(() => {
    stop();
    forgetClick();
  });

  return { state, start };
};
