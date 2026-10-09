import { createEffect, createSignal, on, onCleanup } from "solid-js";
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
  /** How far the dragged rows move from their measured place so their top stays `grabOffset` above the pointer. */
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
  /** The dragged row's own top once the Live lines above it have hidden: what `lift` offsets from. */
  liftedTop: number;
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
  const liftedTop = Math.min(...liftedBounds.map((bounds) => bounds.top));

  return {
    nodes,
    rows: remaining,
    bounds: remaining.map((row) => boundsOf(row.node.id)),
    lifted: new Set(lifted),
    liftedTop,
    origin: rows.findIndex((row) => row.node.id === id),
    room: Math.max(...liftedBounds.map((bounds) => bounds.bottom)) - liftedTop,
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
  /** Names the rows and where they sit; a drag ends without dropping when it changes after the rows were measured. */
  layoutKey: () => string;
  /** Read after `onDragging(id)` has run, so the rows are the ones laid out for the drag. */
  rows: () => readonly NodeRow[];
  /** The id from the first move past a click until release, then `null`; the Rail lays its rows out for the drag in between, before they are measured. */
  onDragging: (id: string | null) => void;
  /** A drag cannot start while this is false, and a release while it is false drops nothing. */
  enabled: () => boolean;
  onDrop: (id: string, drop: Drop) => void;
}): RailDrag => {
  const [state, setState] = createSignal<DragState | null>(null);

  let stop: () => void = () => {};

  let measuredLayout: string | null = null;

  let forgetClick: () => void = () => {};

  const start = (id: string, press: PointerEvent): void => {
    const container = source.container();

    if (!container || press.button !== 0 || !source.enabled()) return;

    stop();

    const nodes = [...source.nodes()];
    let snapshot: Snapshot | null = null;

    // How far below the row's own top the pointer grabbed it, measured before the drag hides the Live lines above
    // it: the row keeps this same offset from the pointer for the rest of the drag, however much those rows
    // shrink once the drag starts (docs/motion.md, "Drag in the rail: follows pointer").
    const pressedRow = container.querySelector(`[data-id="${CSS.escape(id)}"]`);
    const grabOffset = press.clientY - (pressedRow?.getBoundingClientRect().top ?? press.clientY);

    const follow = (move: PointerEvent) => {
      if (!snapshot && Math.hypot(move.clientX - press.clientX, move.clientY - press.clientY) < THRESHOLD_PX) return;

      if (!snapshot) source.onDragging(id);

      snapshot ??= measured(container, nodes, source.rows(), id);
      measuredLayout = source.layoutKey();

      const base = container.getBoundingClientRect();
      const lift = move.clientY - base.top - grabOffset - snapshot.liftedTop;

      setState(stateAt(snapshot, id, base, { x: move.clientX, y: move.clientY }, lift));
    };

    const release = () => {
      const landed = state();

      stop();

      if (landed) forgetClick = swallowNextClick();

      if (landed && source.enabled() && !isInPlace(nodes, id, landed.drop)) source.onDrop(id, landed.drop);
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
      measuredLayout = null;
    };

    window.addEventListener("pointermove", follow);
    window.addEventListener("pointerup", release);
    window.addEventListener("pointercancel", stop);
    window.addEventListener("keydown", cancelOnEscape);
  };

  createEffect(
    on(
      source.layoutKey,
      (layout) => {
        if (measuredLayout !== null && layout !== measuredLayout) stop();
      },
      { defer: true },
    ),
  );

  onCleanup(() => {
    stop();
    forgetClick();
  });

  return { state, start };
};
