import { fireEvent, screen } from "@solidjs/testing-library";
import { vi } from "vitest";
import { rowOf } from "./railFixture";

const ROW_HEIGHT = 20;

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
