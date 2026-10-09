import { fireEvent, screen } from "@solidjs/testing-library";
import { vi } from "vitest";
import { rowOf } from "./railFixture";

const ROW_HEIGHT = 20;

/** The extra height a row's Live line adds (motion.md "follows pointer": hiding it at drag start must not move the dragged row off the pointer). */
const LIVE_HEIGHT = 16;

/** One indent step (2ch) in pixels. */
const STEP = 20;

const rect = (left: number, top: number, width: number, height: number): DOMRect => new DOMRect(left, top, width, height);

/** jsdom has no layout: rows stack at `ROW_HEIGHT` in document order, the Rail starts at the origin, and anything else (the 2ch probe) is `STEP` wide. */
export const stubLayout = () =>
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    if (this.getAttribute("role") === "treeitem") {
      const index = screen.getAllByRole("treeitem").indexOf(this);

      return rect(0, index * ROW_HEIGHT, 200, ROW_HEIGHT);
    }

    if (this.classList.contains("rail-tree")) return rect(0, 0, 200, 400);

    return rect(0, 0, STEP, ROW_HEIGHT);
  });

/**
 * Like `stubLayout`, but a row showing its Live line (`.live`) is `LIVE_HEIGHT` taller, read fresh from the DOM on
 * every call: a row's height can shrink mid-drag once U7's Live line hides (motion.md "follows pointer"), and later rows' tops must follow.
 */
export const stubVariableLayout = () =>
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    if (this.getAttribute("role") === "treeitem") {
      const rows = screen.getAllByRole("treeitem");
      const index = rows.indexOf(this);
      const heightOf = (row: HTMLElement) => ROW_HEIGHT + (row.querySelector(".live") ? LIVE_HEIGHT : 0);
      const top = rows.slice(0, index).reduce((sum, row) => sum + heightOf(row), 0);

      return rect(0, top, 200, heightOf(this));
    }

    if (this.classList.contains("rail-tree")) return rect(0, 0, 200, 2000);

    return rect(0, 0, STEP, ROW_HEIGHT);
  });

/** The pointer at the middle of the gap below row `slot - 1`, `depth` steps in. */
export const pointerAt = (slot: number, depth: number) => ({
  clientX: depth * STEP,
  clientY: slot * ROW_HEIGHT,
});

export const dragFrom = (name: string, to: { clientX: number; clientY: number }) => {
  fireEvent.pointerDown(rowOf(name), { button: 0, clientX: 5, clientY: 5 });
  fireEvent.pointerMove(window, to);
};

export const release = (at: { clientX: number; clientY: number }) => fireEvent.pointerUp(window, at);
