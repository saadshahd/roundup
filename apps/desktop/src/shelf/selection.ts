import { createEffect, createSignal, on, onCleanup, useContext } from "solid-js";
import { ShelfActions } from "./BulkDelete";
import type { Accessor } from "solid-js";

/** U458: identities, rather than positions, survive a list refetch. */
export const createSelection = <Id>(ids: Accessor<readonly Id[]>, context: Accessor<string>) => {
  const host = useContext(ShelfActions);
  const [selected, setSelected] = createSignal<readonly Id[]>([]);
  let anchor: Id | undefined;
  const rows = new Map<Id, HTMLButtonElement>();

  const clear = () => {
    setSelected([]);
    anchor = undefined;
  };

  createEffect(on(() => `${context()}:${host?.open()}`, clear, { defer: true }));
  createEffect(() => {
    const visible = ids();
    setSelected((held) => held.filter((id) => visible.includes(id)));

    for (const id of rows.keys()) if (!visible.includes(id)) rows.delete(id);

    if (anchor !== undefined && !visible.includes(anchor)) anchor = undefined;
  });
  onCleanup(clear);

  const toggle = (id: Id) => {
    setSelected((held) => held.includes(id) ? held.filter((other) => other !== id) : [...held, id]);
    anchor = id;
  };

  const range = (id: Id) => {
    const order = ids();
    const start = anchor === undefined ? -1 : order.indexOf(anchor);
    const end = order.indexOf(id);
    setSelected(start < 0 ? [id] : [...new Set(order.slice(Math.min(start, end), Math.max(start, end) + 1))]);
  };

  const click = (id: Id, event: MouseEvent, open: () => void) => {
    if (event.shiftKey) range(id);
    else if (event.metaKey) toggle(id);
    else {
      clear();
      anchor = id;
      open();
    }
  };

  const key = (id: Id, event: KeyboardEvent) => {
    if (event.ctrlKey || event.altKey) return;

    if (event.key === "Enter" && event.metaKey && !event.shiftKey) {
      event.preventDefault();

      if (!event.repeat) toggle(id);

      return;
    }

    if (event.metaKey || (event.key !== "ArrowUp" && event.key !== "ArrowDown")) return;
    event.preventDefault();
    const order = ids();
    const next = order[order.indexOf(id) + (event.key === "ArrowUp" ? -1 : 1)];

    if (next === undefined) return;

    if (event.shiftKey) {
      anchor ??= id;
      range(next);
    }

    rows.get(next)?.focus();
  };

  return {
    selected, clear, click, key, toggle,
    cancel: () => { const row = rows.get(selected()[0]!); clear(); row?.focus(); },
    replace: (values: readonly Id[]) => setSelected([...new Set(values)]),
    register: (id: Id, row: HTMLButtonElement | null) => row ? rows.set(id, row) : rows.delete(id),
  };
};
