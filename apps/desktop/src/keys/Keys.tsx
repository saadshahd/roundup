import { onCleanup } from "solid-js";
import { useConnectedProject } from "../state/connectedProject";
import { placeOf } from "./places";
import { railStep } from "./railStep";

const isRow = (target: EventTarget | null): target is HTMLElement =>
  target instanceof HTMLElement && target.dataset.id !== undefined;

/** `aria-expanded` is set only on a Workstream's row (U8's `▾`/`▸`); anything else has no fold to read. */
const expandedOf = (row: HTMLElement): boolean | null => {
  const value = row.getAttribute("aria-expanded");

  return value === null ? null : value === "true";
};

/** The row right after `row` in Rail order, one level deeper: `→`'s first child (U41). A child folded into U8's
 * `✓ n done` line is never it, because the Rail renders no row for it until the fold opens. */
const firstVisibleChildOf = (row: HTMLElement): string | undefined => {
  let next = row.nextElementSibling;

  while (next instanceof HTMLElement && next.dataset.pads !== undefined) next = next.nextElementSibling;

  if (!(next instanceof HTMLElement) || next.dataset.id === undefined) return undefined;

  const level = Number(row.getAttribute("aria-level"));
  const nextLevel = Number(next.getAttribute("aria-level"));

  return nextLevel > level ? next.dataset.id : undefined;
};

const rowElement = (id: string): HTMLElement | null =>
  document.querySelector<HTMLElement>(`[role="tree"] [data-id="${id}"]`);

const focusTabbableRow = (): void => document.querySelector<HTMLElement>('[role="tree"] [tabindex="0"]')?.focus();

const focusPane = (): void =>
  document.querySelector<HTMLElement>(".pane-screen textarea, .pane-screen [tabindex]")?.focus();

/** `⌘1`, `⌘2`, `←`, `→` and `F2`, the Rail keys U31 leaves to U41 (GLOSSARY.md, Rail). No UI of its own. */
export const Keys = () => {
  const { rail } = useConnectedProject();

  const onArrow = (row: HTMLElement, direction: "left" | "right"): void => {
    const id = row.dataset.id;

    if (id === undefined) return;

    const control = row.querySelector<HTMLElement>("[data-pads-control]");
    const padsShown = control?.getAttribute("aria-expanded") === "true";

    /** U58: `→` on a leaf Agent opens its Pads, and `←` folds them before it selects the parent. */
    if (control && (direction === "left" ? padsShown : expandedOf(row) === null && !padsShown && firstVisibleChildOf(row) === undefined)) {
      control.click();

      return;
    }

    const step = railStep(rail.nodes, id, direction, expandedOf(row), firstVisibleChildOf(row));

    if (step.kind === "collapse" || step.kind === "expand") {
      row.querySelector<HTMLElement>('button[aria-label="collapse"]')?.click();
    } else if (step.kind === "select") {
      rowElement(step.id)?.focus();
      rail.select(step.id);
    }
  };

  const onRename = (row: HTMLElement): void => {
    row.querySelector(".name")?.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
  };

  const onKey = (press: KeyboardEvent): void => {
    if (press.metaKey && !press.ctrlKey && !press.altKey && !press.shiftKey && (press.key === "1" || press.key === "2")) {
      press.preventDefault();
      (press.key === "1" ? focusTabbableRow : focusPane)();

      return;
    }

    if (press.metaKey || press.ctrlKey || press.altKey || press.shiftKey) return;

    if (placeOf(press.target instanceof Element ? press.target : null) !== "rail") return;

    if (!isRow(press.target)) return;

    if (press.key === "ArrowLeft" || press.key === "ArrowRight") {
      press.preventDefault();
      onArrow(press.target, press.key === "ArrowLeft" ? "left" : "right");
    } else if (press.key === "F2") {
      press.preventDefault();
      onRename(press.target);
    }
  };

  document.addEventListener("keydown", onKey);
  onCleanup(() => document.removeEventListener("keydown", onKey));

  return null;
};
