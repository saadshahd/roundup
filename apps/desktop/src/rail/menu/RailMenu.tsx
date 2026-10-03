import { createEffect, onCleanup, onMount, Show } from "solid-js";
import type { RailMenuModel } from "./model";

export const RailMenu = (props: { menu: RailMenuModel }) => {
  let element: HTMLDivElement | undefined;

  onMount(() => {
    const dismiss = (press: PointerEvent) => {
      if (element && press.target instanceof Node && !element.contains(press.target)) props.menu.close();
    };

    document.addEventListener("pointerdown", dismiss);
    onCleanup(() => document.removeEventListener("pointerdown", dismiss));
  });

  createEffect(() => {
    const current = props.menu.state();

    if (current) queueMicrotask(() => element?.querySelector<HTMLButtonElement>("[role=menuitem]")?.focus());
  });

  const closeAndFocusRow = () => {
    const id = props.menu.state()?.node.id;

    props.menu.close();
    queueMicrotask(() => {
      document.querySelectorAll<HTMLElement>("[role=treeitem]").forEach((row) => {
        if (row.dataset.id === id) row.focus();
      });
    });
  };

  const onKeyDown = (press: KeyboardEvent) => {
    if (press.key === "Escape") {
      press.preventDefault();
      closeAndFocusRow();

      return;
    }

    const items = [...(element?.querySelectorAll<HTMLButtonElement>("[role=menuitem]") ?? [])];
    const index = items.findIndex((item) => item === document.activeElement);

    if (press.key === "ArrowDown" || press.key === "ArrowUp") {
      press.preventDefault();
      items[Math.max(0, Math.min(items.length - 1, index + (press.key === "ArrowDown" ? 1 : -1)))]?.focus();
    } else if (press.key === "Enter") {
      press.preventDefault();
      items[Math.max(0, index)]?.click();
    }
  };

  return (
    <Show when={props.menu.state()}>
      {(current) => (
        <div
          ref={(node) => { element = node; }}
          class="rail-menu"
          role="menu"
          aria-label={`actions for ${current().node.name}`}
          style={{ left: `${current().x}px`, top: `${current().y}px` }}
          onKeyDown={onKeyDown}
          onPointerDown={(press) => press.stopPropagation()}
        >
          <Show
            when={current().confirming}
            fallback={
              <>
                <Show when={props.menu.canStop(current())}>
                  <button role="menuitem" onClick={props.menu.chooseStop}>stop</button>
                </Show>
                <button role="menuitem" onClick={props.menu.chooseRemove}>remove</button>
              </>
            }
          >
            <span class="rail-menu-question">{`stop and remove ${current().node.name}?`}</span>
            <button role="menuitem" onClick={props.menu.chooseRemove}>remove</button>
            <button role="menuitem" onClick={closeAndFocusRow}>keep</button>
          </Show>
        </div>
      )}
    </Show>
  );
};
