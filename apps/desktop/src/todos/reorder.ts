import { createEffect, on, onCleanup } from "solid-js";
import type { Accessor } from "solid-js";
import type { Todo } from "@contracts/todo/Todo";
import { swallowNextClick } from "../rail/drag";

/** U156: the layout animation of a row that changed place, the one U53 gives a row below a completed one. */
export const REORDER_MS = 150;

export const REORDER_EASING = "ease-out";

/** U155: a press that moves less than this is the click of U17. */
const THRESHOLD_PX = 4;

/** U155: `todo.reorder`'s `before` for putting `id` at index `to` of the list `listIds`: the next row of the list, else the Todo after the list's last row in `all`, else null. */
export const beforeOf = (all: readonly Todo[], listIds: readonly number[], id: number, to: number): number | null => {
  const others = listIds.filter((other) => other !== id);
  const next = others[to];

  if (next !== undefined) return next;

  const last = others.at(-1);
  const rest = all.filter((todo) => todo.id !== id);

  return (last === undefined ? undefined : rest[rest.findIndex((todo) => todo.id === last) + 1])?.id ?? null;
};

/** The vertical offset a computed `transform` holds, 0 for none. */
const offsetOf = (el: Element): number => {
  const match = /matrix\(([^)]+)\)/.exec(getComputedStyle(el).transform);

  return match ? Number(match[1]!.split(",")[5]) || 0 : 0;
};

export type Reorder = {
  /** The row's frame, for measuring and moving it. */
  frame: (id: number, el: HTMLElement | null) => void;
  /** The row button, which keeps focus after a keyboard move. */
  button: (id: number, el: HTMLButtonElement | null) => void;
  /** `⌥↑` and `⌥↓` on a row button; `fail` takes the message of a failed call, else null. */
  key: (id: number, event: KeyboardEvent, fail: (message: string | null) => void) => void;
  /** A press on a row button: past `THRESHOLD_PX` of vertical movement it drags the row. */
  press: (id: number, event: PointerEvent, fail: (message: string | null) => void) => void;
};

/** U155, U156: moves a Todo row inside its own list, by pointer or keyboard, and animates the rows that change place when the refetched list arrives. */
export const createReorder = (source: {
  /** The ids of the list's rows, top to bottom. */
  ids: Accessor<readonly number[]>;
  /** Every Todo, in the Daemon's order. */
  all: Accessor<readonly Todo[]>;
  call: (params: { id: number; before: number | null }) => Promise<string | null>;
  reducedMotion: Accessor<boolean>;
}): Reorder => {
  const frames = new Map<number, HTMLElement>();
  const buttons = new Map<number, HTMLButtonElement>();
  const tops = new Map<number, number>();
  let refocus: number | null = null;
  let stop: () => void = () => {};

  const measure = () => frames.forEach((el, id) => tops.set(id, el.offsetTop));

  /** Clears the offsets a drag left and slides each row from where it showed to where the list now puts it. */
  const settle = () => {
    for (const [id, el] of frames) {
      const top = el.offsetTop;
      const was = tops.get(id);
      const shown = offsetOf(el);

      el.style.transform = "";
      tops.set(id, top);

      if (was === undefined || source.reducedMotion() || !("animate" in el)) continue;

      const delta = was + shown - top;

      el.getAnimations().forEach((animation) => animation.cancel());

      if (Math.abs(delta) >= 1) el.animate([{ transform: `translateY(${delta}px)` }, { transform: "none" }], { duration: REORDER_MS, easing: REORDER_EASING });
    }

    if (refocus !== null) buttons.get(refocus)?.focus();

    refocus = null;
  };

  createEffect(on(source.ids, settle));
  onCleanup(() => stop());

  const run = async (id: number, to: number, fail: (message: string | null) => void) => {
    const message = await source.call({ id, before: beforeOf(source.all(), source.ids(), id, to) });

    fail(message);

    if (message !== null) {
      refocus = null;
      settle();
    }
  };

  return {
    frame: (id, el) => {
      if (el) frames.set(id, el);
      else {
        frames.delete(id);
        tops.delete(id);
      }
    },
    button: (id, el) => {
      if (el) buttons.set(id, el);
      else buttons.delete(id);
    },
    key: (id, event, fail) => {
      if (!event.altKey || (event.key !== "ArrowUp" && event.key !== "ArrowDown")) return;

      event.preventDefault();

      const list = source.ids();
      const to = list.indexOf(id) + (event.key === "ArrowUp" ? -1 : 1);

      if (to < 0 || to >= list.length) return;

      measure();
      refocus = id;
      void run(id, to, fail);
    },
    press: (id, press, fail) => {
      const el = frames.get(id);

      if (!el || press.button !== 0) return;

      stop();

      let dragging = false;
      let to = 0;
      const list = source.ids();
      const origin = list.indexOf(id);

      const follow = (move: PointerEvent) => {
        const dy = move.clientY - press.clientY;

        if (!dragging) {
          if (Math.abs(dy) < THRESHOLD_PX) return;

          dragging = true;
          measure();
        }

        const room = el.offsetHeight + (parseFloat(getComputedStyle(el.parentElement!).rowGap) || 0);
        const centre = el.offsetTop + dy + el.offsetHeight / 2;
        const others = list.filter((other) => other !== id);

        to = others.filter((other) => {
          const row = frames.get(other)!;

          return row.offsetTop + row.offsetHeight / 2 < centre;
        }).length;
        el.style.transform = `translateY(${dy}px)`;
        list.forEach((other, index) => {
          if (other === id) return;

          const shift = to < origin && index >= to && index < origin ? room : to > origin && index > origin && index <= to ? -room : 0;

          frames.get(other)!.style.transform = shift === 0 ? "" : `translateY(${shift}px)`;
        });
      };

      const finish = (drop: boolean, release?: PointerEvent) => {
        stop();

        if (!dragging) return;

        swallowNextClick();

        const bounds = el.parentElement!.getBoundingClientRect();
        const inside = release !== undefined && release.clientY >= bounds.top && release.clientY <= bounds.bottom && release.clientX >= bounds.left && release.clientX <= bounds.right;

        if (drop && inside && to !== origin) void run(id, to, fail);
        else settle();
      };

      const release = (up: PointerEvent) => finish(true, up);
      const cancel = () => finish(false);

      const escape = (key: KeyboardEvent) => {
        if (key.key === "Escape") cancel();
      };

      stop = () => {
        window.removeEventListener("pointermove", follow);
        window.removeEventListener("pointerup", release);
        window.removeEventListener("pointercancel", cancel);
        window.removeEventListener("keydown", escape);
        stop = () => {};
      };

      window.addEventListener("pointermove", follow);
      window.addEventListener("pointerup", release);
      window.addEventListener("pointercancel", cancel);
      window.addEventListener("keydown", escape);
    },
  };
};
