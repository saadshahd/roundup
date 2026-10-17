import { createEffect, createSignal, on } from "solid-js";
import type { Accessor } from "solid-js";
import type { Todo } from "@contracts/todo/Todo";
import type { Sortable } from "../ui/sortable";
import type { SortedRow } from "./Todos";
import { beforeOf, REORDER_EASING, REORDER_MS } from "./beforeOf";

type Parts = Parameters<Parameters<typeof Sortable.Item>[0]["children"]>[0];

/** A Rail Home a Shelf row can be dropped on: `home` is a Workstream's Rail id, or null for the Project root. */
export type HomeTarget = { element: HTMLElement; home: string | null };

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
}) => {
  const frames = new Map<number, HTMLElement>();
  const buttons = new Map<number, HTMLButtonElement>();
  const fails = new Map<number, (message: string | null) => void>();
  const tops = new Map<number, number>();
  let refocus: number | null = null;
  const [movedFocus, setMovedFocus] = createSignal<{ id: number; home: string | null; element: HTMLElement } | null>(null);

  /** A row's top on screen, any slide or drag offset in effect included: the offset parent of a nested row is its own Leaf, so `offsetTop` cannot tell where it is. */
  const topOf = (el: HTMLElement) => el.getBoundingClientRect().top;

  const measure = () => frames.forEach((el, id) => tops.set(id, topOf(el)));

  /** U156: slides each row from where it showed (mid-slide included) to where the refetched list puts it. */
  const settle = () => {
    for (const [id, el] of frames) {
      if ("getAnimations" in el) el.getAnimations().forEach((animation) => animation.cancel());

      const top = topOf(el);
      const was = tops.get(id);

      tops.set(id, top);

      if (was === undefined || source.reducedMotion() || !("animate" in el) || Math.abs(was - top) < 1) continue;
      el.animate([{ transform: `translateY(${was - top}px)` }, { transform: "none" }], { duration: REORDER_MS, easing: REORDER_EASING });
    }

    if (refocus !== null) buttons.get(refocus)?.focus();

    refocus = null;
  };

  createEffect(on(() => source.ids().join(), settle, { defer: true }));
  createEffect(() => {
    const all = source.all();
    const ids = source.ids();
    const request = movedFocus();

    if (!request || all.find((todo) => todo.id === request.id)?.home !== request.home) return;
    setMovedFocus(null);
    queueMicrotask(() => {
      if (document.activeElement === document.body || document.activeElement === null) {
        (ids.includes(request.id) ? buttons.get(request.id) : request.element)?.focus();
      }
    });
  });

  /** One `todo.reorder` putting `id` at index `to` of this list. */
  const move = async (id: number, to: number) => {
    const message = await source.call({ id, before: beforeOf(source.all(), source.ids(), id, to) });

    fails.get(id)?.(message);

    if (message !== null) {
      refocus = null;
      buttons.get(id)?.focus();
    }
  };

  const step = (id: number, by: -1 | 1) => {
    const to = source.ids().indexOf(id) + by;

    if (to < 0 || to >= source.ids().length) return;

    measure();
    refocus = id;
    void move(id, to);
  };

  return {
    /** The Sortable's new order of the ids: the row it moved goes to its index. */
    order: (next: readonly string[]) => {
      const before = source.ids().map(String);
      const at = next.findIndex((id, index) => id !== before[index]);
      const moved = [next[at], before[at]].find((id) => id !== undefined && before.filter((other) => other !== id).join() === next.filter((other) => other !== id).join());

      if (moved === undefined) return;
      measure();
      refocus = Number(moved);
      void move(refocus, next.indexOf(moved));
    },
    /** What `ui/Sortable.Root` needs to drop a row on a Rail Home, else undefined. */
    external: source.home ? {
      attribute: "data-todo-drop",
      target: (id: string, x: number, y: number) => source.home!.target(Number(id), x, y),
      onDropOutside: (id: string, target: HomeTarget) => {
        const todo = Number(id);

        setMovedFocus({ id: todo, home: target.home, element: target.element });
        void source.home!.move(todo, target.home).then((message) => {
          fails.get(todo)?.(message);

          if (message !== null) setMovedFocus(null);

          // A refetch can remove the focused row before the RPC resolves.
          if (document.activeElement === document.body || document.activeElement === null) target.element.focus();
        });
      },
    } : undefined,
    row: (id: number, parts: Parts): SortedRow => ({
      frame: (el) => {
        if (el) {
          frames.set(id, el);
          parts.ref(el);
        } else {
          frames.delete(id);
          tops.delete(id);
        }
      },
      button: (el) => {
        if (el) buttons.set(id, el);
        else {
          buttons.delete(id);
          fails.delete(id);
        }

        parts.handleRef(el ?? undefined);
      },
      fail: (show) => fails.set(id, show),
      step: (by) => step(id, by),
      key: (event) => {
        if (!event.altKey || (event.key !== "ArrowUp" && event.key !== "ArrowDown")) return;

        event.preventDefault();
        step(id, event.key === "ArrowUp" ? -1 : 1);
      },
    }),
  };
};
