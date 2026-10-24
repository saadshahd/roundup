import { captureFocus } from "../keys/focus";
import { createComputed, createEffect, createSignal, on, onCleanup } from "solid-js";
import type { Accessor } from "solid-js";
import type { Todo } from "@contracts/todo/Todo";
import type { Sortable } from "../ui/sortable";
import type { SortedRow } from "./OpenRow";
import type { Actor } from "@contracts/Actor";
import { sameOwner } from "../shelf/scope";
import { beforeOf, slideOf } from "./beforeOf";

type Parts = Parameters<Parameters<typeof Sortable.Item>[0]["children"]>[0];

/** A Rail Home a Shelf row can be dropped on: `home` is a Workstream's Rail id, or null for the Project root. */
export type HomeTarget = { element: HTMLElement; home: string | null };

/** U3718: an owner's group a Shelf row can be dropped on; `owner` is null for Unassigned. */
export type OwnerTarget = { element: HTMLElement; attribute: string; owner: Actor | null };

/** U490a, U155, U156, U362: the rows of one list of open Todos ordered through `ui/Sortable`: one `todo.reorder` per move, focus kept on the moved row, and the rows that change place slide by U156. */
export const createSortedList = (source: {
  /** The ids of the list's rows, top to bottom. */
  ids: Accessor<readonly number[]>;
  /** Every Todo, in the Daemon's order. */
  all: Accessor<readonly Todo[]>;
  call: (params: { id: number; before: number | null }) => Promise<string | null>;
  reducedMotion: Accessor<boolean>;
  /** U362: only the Shelf's rows can be dropped on a Rail Home. */
  home?: {
    target: (id: number, x: number, y: number) => HomeTarget | null;
    move: (id: number, home: string | null) => Promise<string | null>;
  };
  /** U3718: the Shelf's groups by owner are drop targets too. */
  owner?: {
    target: (id: number, x: number, y: number) => OwnerTarget | null;
    assign: (id: number, owner: Actor | null) => Promise<string | null>;
  };
}) => {
  const frames = new Map<number, HTMLElement>();
  const buttons = new Map<number, HTMLButtonElement>();
  const fails = new Map<number, (message: string | null) => void>();
  const positions = new Map<number, { top: number; left: number }>();
  const animations = new Map<number, Animation>();
  let savedFocus: number | undefined;
  let savedPermission = captureFocus();
  let requestedFocus = captureFocus();
  let scroll: { element: HTMLElement; top: number; left: number } | null = null;
  onCleanup(() => animations.forEach(animation => animation.cancel()));
  let refocus: number | null = null;
  const [movedFocus, setMovedFocus] = createSignal<{ id: number; arrived: (todo: Todo) => boolean; element: HTMLElement; focus: ReturnType<typeof captureFocus> } | null>(null);

  /** A row's top on screen, any slide or drag offset in effect included: the offset parent of a nested row is its own Leaf, so `offsetTop` cannot tell where it is. */
  const positionOf = (el: HTMLElement) => {
    const row = el.getBoundingClientRect();
    const box = row.width > 0 && row.height > 0 ? row : el.closest("[data-shelf-group]")?.querySelector(".shelf-group-head")?.getBoundingClientRect();

    return box && box.width > 0 && box.height > 0 ? { top: box.top, left: box.left } : undefined;
  };

  const measure = () => {
    savedPermission = captureFocus();
    savedFocus = [...buttons].find(([, button]) => button === document.activeElement)?.[0];
    const area = frames.values().next().value?.closest<HTMLElement>(".shelf-area");

    scroll = area ? { element: area, top: area.scrollTop, left: area.scrollLeft } : null;
    frames.forEach((el, id) => {
      const position = positionOf(el);

      if (position) positions.set(id, position);
      // A replacement frame may be registered before insertion; its outgoing position still belongs to this row.
    });
  };

  /** U156: slides each row from where it showed (mid-slide included) to where the refetched list puts it. */
  const settle = () => {
    if (scroll) {
      scroll.element.scrollTop = scroll.top;
      scroll.element.scrollLeft = scroll.left;
      scroll = null;
    }

    for (const [id, el] of frames) {
      animations.get(id)?.cancel();
      animations.delete(id);

      if ("getAnimations" in el) el.getAnimations().forEach((animation) => animation.cancel());

      const position = positionOf(el);
      const was = positions.get(id);

      if (!position) {
        positions.delete(id);
        continue;
      }

      positions.set(id, position);

      if (was === undefined || source.reducedMotion() || !("animate" in el)) continue;
      const x = was.left - position.left;
      const y = was.top - position.top;

      if (Math.abs(x) < 1 && Math.abs(y) < 1) continue;
      const animation = el.animate([{ transform: `translate(${x}px, ${y}px)`, opacity: 0.8 }, { transform: "none", opacity: 1 }], slideOf());

      animations.set(id, animation);
      animation.onfinish = () => { if (animations.get(id) === animation) animations.delete(id); };
    }

    for (const id of positions.keys()) if (!source.ids().includes(id)) {
      positions.delete(id);
      animations.get(id)?.cancel();
      animations.delete(id);
    }

    if (refocus !== null) requestedFocus(() => buttons.get(refocus!)?.focus({ preventScroll: true }));
    else if (savedFocus !== undefined && (document.activeElement === document.body || document.activeElement === null)) savedPermission(() => buttons.get(savedFocus!)?.focus({ preventScroll: true }));
    savedFocus = undefined;

    refocus = null;
  };

  // U3010c: any change of the list (an insertion, a completion) first reads where each row shows now, before the DOM follows; a measure taken at the last move would be stale.
  const placement = () => source.ids().map(id => {
    const todo = source.all().find(todo => todo.id === id);

    return `${id}:${todo?.owner?.kind ?? ""}:${todo?.owner?.id ?? ""}`;
  }).join();

  createComputed(on(placement, measure, { defer: true }));
  createEffect(on(placement, () => queueMicrotask(settle), { defer: true }));
  // U3010c: reduced motion switched on mid-slide settles every row now, with no replay.
  createEffect(on(source.reducedMotion, (reduced) => { if (reduced) frames.forEach((el) => "getAnimations" in el && el.getAnimations().forEach((animation) => animation.cancel())); }, { defer: true }));
  createEffect(() => {
    const all = source.all();
    const ids = source.ids();
    const request = movedFocus();

    const moved = all.find((todo) => todo.id === request?.id);

    if (!request || !moved || !request.arrived(moved)) return;
    setMovedFocus(null);
    queueMicrotask(() => {
      if (document.activeElement === document.body || document.activeElement === null) {
        const row = ids.includes(request.id) ? buttons.get(request.id) ?? document.querySelector<HTMLElement>(`[data-todo="${request.id}"] .row-head button`) : request.element;

        request.focus(() => {
          row?.focus();

          // A folded owner group keeps its moved row hidden; its disclosure keeps focus reachable.
          if (document.activeElement === document.body) request.element.querySelector<HTMLElement>('[data-part="branch-control"]')?.focus();
        });
      }
    });
  });

  /** One `todo.reorder` putting `id` at index `to` of this list. */
  const move = async (id: number, to: number, ids = source.ids()) => {
    const focus = requestedFocus;
    const message = await source.call({ id, before: beforeOf(source.all(), ids, id, to) });

    fails.get(id)?.(message);

    if (message !== null) {
      refocus = null;
      focus(() => buttons.get(id)?.focus());
    }
  };

  const step = (id: number, by: -1 | 1, ids = source.ids()) => {
    const to = ids.indexOf(id) + by;

    if (to < 0 || to >= ids.length) return;

    measure();
    requestedFocus = captureFocus();
    refocus = id;
    void move(id, to, ids);
  };

  return {
    /** U3710: one `todo.reorder` for a drop that changed place, its failure shown on the row. */
    reorderTo: async (id: number, before: number | null) => {
      measure();
      requestedFocus = captureFocus();
      refocus = id;

      const message = await source.call({ id, before });

      fails.get(id)?.(message);

      if (message !== null) refocus = null;

      return message;
    },
    /** U3711: shows a refusal on the row. */
    show: (id: number, message: string | null) => fails.get(id)?.(message),
    /** The Sortable's new order of the ids: the row it moved goes to its index. */
    order: (next: readonly string[]) => {
      const before = source.ids().map(String).filter(id => next.includes(id));
      const at = next.findIndex((id, index) => id !== before[index]);
      const moved = [next[at], before[at]].find((id) => id !== undefined && before.filter((other) => other !== id).join() === next.filter((other) => other !== id).join());

      if (moved === undefined) return;
      measure();
      requestedFocus = captureFocus();
      refocus = Number(moved);
      void move(refocus, next.indexOf(moved), before.map(Number));
    },
    /** What `ui/Sortable.Root` needs to drop a row on a Rail Home, else undefined. */
    external: source.home || source.owner ? {
      attribute: "data-todo-drop",
      target: (id: string, x: number, y: number): HomeTarget | OwnerTarget | null => source.home?.target(Number(id), x, y) ?? source.owner?.target(Number(id), x, y) ?? null,
      onDropOutside: (id: string, target: HomeTarget | OwnerTarget) => {
        const focus = captureFocus();
        const todo = Number(id);

        if ("owner" in target) {
          const owner = target.owner;

          setMovedFocus({ id: todo, arrived: (moved) => sameOwner(moved.owner, owner), element: target.element, focus });
          void source.owner!.assign(todo, owner).then((message) => {
            fails.get(todo)?.(message);

            if (message !== null) {
              setMovedFocus(null);
              focus(() => buttons.get(todo)?.focus());
            }
          });

          return;
        }

        setMovedFocus({ id: todo, arrived: (moved) => moved.home === target.home, element: target.element, focus });
        void source.home!.move(todo, target.home).then((message) => {
          fails.get(todo)?.(message);

          if (message !== null) setMovedFocus(null);

          // A refetch can remove the focused row before the RPC resolves.
          if (document.activeElement === document.body || document.activeElement === null) focus(() => target.element.focus());
        });
      },
    } : undefined,
    row: (id: number, parts: Parts, ids: Accessor<readonly number[]> = source.ids): SortedRow => {
      let frame: HTMLElement | null = null;
      let button: HTMLButtonElement | null = null;

      return {
      frame: (el) => {
        if (el) {
          const outgoing = frames.get(id);
          const position = outgoing && positionOf(outgoing);

          if (position) positions.set(id, position);
          frame = el;
          frames.set(id, el);
          parts.ref(el);
        } else if (frames.get(id) === frame) {
          // An owner change replaces the row before measure; retain its mid-glide position for the new frame.
          const position = frame && positionOf(frame);

          if (position) positions.set(id, position);
          frames.delete(id);
        }
      },
      button: (el) => {
        if (el) {
          button = el;
          buttons.set(id, el);
        } else if (buttons.get(id) === button) {
          buttons.delete(id);
          fails.delete(id);
        }

        parts.handleRef(el ?? undefined);
      },
      fail: (show) => fails.set(id, show),
      step: (by) => step(id, by, ids()),
      can: (by) => {
        const list = ids();
        const to = list.indexOf(id) + by;

        return list.includes(id) && to >= 0 && to < list.length;
      },
      key: (event) => {
        if (!event.altKey || (event.key !== "ArrowUp" && event.key !== "ArrowDown")) return;

        event.preventDefault();
        step(id, event.key === "ArrowUp" ? -1 : 1, ids());
      },
      };
    },
  };
};
