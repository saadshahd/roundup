import { captureFocus } from "../keys/focus";
import type { RailState } from "../state/rail";
import type { App, AppSeam, FileDrop } from "./seam";

/** U348: how long a drag holds over a Rail row or a Project tab before it opens; policy, above a pass at drag speed and below a wait that reads as stuck. */
export const HOLD_MS = 600;

type Listener = (drop: FileDrop) => void;

const SELECTOR = { row: ".rail-row[data-id]", tab: ".project-tab[data-path]" } as const;

const inside = (box: DOMRect, x: number, y: number): boolean => x >= box.left && x < box.right && y >= box.top && y < box.bottom;

const centre = (box: DOMRect) => ({ x: box.left + box.width / 2, y: box.top + box.height / 2 });

/** The element of `selector` whose rectangle, clipped to its `container`, holds the point; a row or tab scrolled out of view is none. */
const under = (selector: string, container: string, x: number, y: number): HTMLElement | null => {
  const clip = document.querySelector(container)?.getBoundingClientRect();

  if (clip === undefined || !inside(clip, x, y)) return null;

  return [...document.querySelectorAll<HTMLElement>(selector)].find((element) => inside(element.getBoundingClientRect(), x, y)) ?? null;
};

/**
 * U348: the drag of files over the window. It reads the App seam's `onFileDrop`, shows the drop targets, opens a row or tab hovered for HOLD_MS, and on a drop
 * selects the target and hands the drop to the pane it lands in. `forProject` returns calls whose `onFileDrop` is this router's feed,
 * so a Pane hears what the router lets through.
 */
export const createDropTargets = (
  app: App,
  host: { rail: () => RailState | undefined; select: (path: string) => void },
) => {
  const feed = new Set<Listener>();
  let marked = new Map<Element, string>();
  let kept: { element: HTMLElement; timer: ReturnType<typeof setTimeout> } | null = null;

  const emit = (drop: FileDrop) => feed.forEach((listener) => listener(drop));

  const cancelHold = () => {
    if (kept) clearTimeout(kept.timer);

    kept = null;
  };

  const mark = (next: Map<Element, string>) => {
    marked.forEach((_, element) => {
      if (!next.has(element)) element.removeAttribute("data-drop");
    });
    next.forEach((value, element) => {
      if (marked.get(element) !== value) element.setAttribute("data-drop", value);
    });
    marked = next;
  };

  const clear = () => {
    cancelHold();
    mark(new Map());
  };

  const selectedNode = () => {
    const rail = host.rail();

    return rail?.nodes.find((node) => node.id === rail.selected());
  };

  /** The pane that takes a drop now: the selected Door's or live Agent's Terminal pane. */
  const receiver = (): Element | null => {
    const node = selectedNode();
    const rail = host.rail();

    if (node === undefined || rail === undefined) return null;

    return rail.exitOf(node) === null ? document.querySelector(".pane-body") : null;
  };

  const open = (element: HTMLElement) => {
    if (element.matches(SELECTOR.tab)) host.select(element.dataset.path ?? "");
    else host.rail()?.select(element.dataset.id ?? null);
  };

  const hit = (x: number, y: number) => under(SELECTOR.row, ".rail", x, y) ?? under(SELECTOR.tab, ".tablist", x, y);

  const over = (drop: FileDrop) => {
    if (drop.x < 0 || drop.y < 0 || drop.x >= window.innerWidth || drop.y >= window.innerHeight) return clear();

    const target = hit(drop.x, drop.y);
    const next = new Map<Element, string>();

    document.querySelectorAll(`${SELECTOR.row}, ${SELECTOR.tab}`).forEach((element) => next.set(element, "target"));

    const pane = receiver();

    if (pane) next.set(pane, "target");

    if (target) next.set(target, "hover");

    mark(next);

    if (kept?.element === target) return;

    cancelHold();

    if (target) {
      const focus = captureFocus();
      kept = { element: target, timer: setTimeout(() => focus(() => open(target)), HOLD_MS) };
    }
  };

  const land = async (drop: FileDrop) => {
    const target = hit(drop.x, drop.y);

    clear();

    if (drop.paths.length === 0) return;

    if (target === null) return emit(drop);

    open(target);

    // U420: a tab delivers to the row it has selected, none if it has none. The selection has re-rendered the centre by the next microtask;
    // the drop is then put where the selected Agent or Workstream takes it.
    await Promise.resolve();

    const box = receiver()?.getBoundingClientRect();

    // U415: a tab that delivers nothing reports the drop where it fell, which no target holds, so the Pane says it could not place it.
    emit(box ? { ...drop, ...centre(box) } : drop);
  };

  const handle: Listener = (drop) => {
    if (drop.phase === "over") {
      over(drop);
      emit(drop);
    } else if (drop.phase === "leave") {
      clear();
      emit(drop);
    } else {
      void land(drop);
    }
  };

  const routed: App = {
    ...app,
    forProject: (path) => {
      const calls: AppSeam = app.forProject(path);

      return {
        ...calls,
        onFileDrop: async (listener) => {
          feed.add(listener);

          return () => feed.delete(listener);
        },
      };
    },
  };

  return { app: routed, start: () => app.onFileDrop(handle) };
};
