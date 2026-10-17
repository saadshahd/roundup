import { modalOpen, registerKeys } from "../keys/registry";
import { copy } from "../copy";
import { Show, type JSX } from "solid-js";
import { Portal } from "solid-js/web";
import { Tooltip } from "../ui/Tooltip";
import "./rowButton.styles.css";

/** A clickable row of the Shelf or a Drawer: plain text, no button chrome; a long title wraps under its `#`, not under the glyph; `hang={false}` is the Shelf's (U359), whose Glyph is its own column. */
export const RowButton = (props: { onClick: (event: MouseEvent) => void; pressed?: boolean | undefined; selectionActive?: boolean | undefined; children: JSX.Element; ref?: (row: HTMLButtonElement) => void; reorderable?: boolean; hang?: boolean; label?: string; tooltip?: string | undefined; onKeyDown?: (event: KeyboardEvent) => void; onPointerDown?: (event: PointerEvent) => void }) => {
  let button: HTMLButtonElement | undefined;

  if (props.onKeyDown) registerKeys({ kind: "layer", element: () => button, active: () => document.activeElement === button && !modalOpen(), modal: false }, [{
    chords: ["Meta+Enter", "ArrowUp", "ArrowDown", "Shift+ArrowUp", "Shift+ArrowDown", "Alt+ArrowUp", "Alt+ArrowDown", "Backspace", "Escape"],
    when: (event) => event.target === button
      && (event.altKey ? props.reorderable === true : props.pressed !== undefined)
      && ((event.key !== "Escape" && event.key !== "Backspace") || props.selectionActive === true),
    run: (event) => props.onKeyDown?.(event),
  }]);

  return (
    <Tooltip.Root closeOnEscape={false} disabled={!props.tooltip}>
      <Tooltip.Trigger asChild={(trigger) => <button data-reorderable={props.reorderable ? "" : undefined} {...trigger({
        type: "button",
        class: "todo-row-button",
        ref: (element: HTMLButtonElement) => { button = element; props.ref?.(element); },
        "aria-label": props.label,
        "aria-description": props.reorderable ? copy.reorder : undefined,
        "aria-pressed": props.pressed,
        onClick: (event: MouseEvent) => props.onClick(event),
        onPointerDown: (event: PointerEvent) => props.onPointerDown?.(event),
        style: props.hang === false ? { display: "block" } : { display: "block", "white-space": "pre-wrap", "padding-left": "var(--todo-row-indent)", "text-indent": "calc(-1 * var(--todo-row-indent))" },
      })}>{props.children}</button>} />
      <Show when={props.tooltip !== undefined}>
        <Portal><Tooltip.Positioner><Tooltip.Content class="todo-tooltip">{props.tooltip}</Tooltip.Content></Tooltip.Positioner></Portal>
      </Show>
    </Tooltip.Root>
  );
};
