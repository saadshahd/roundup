import { render } from "solid-js/web";
import { createSignal, For } from "solid-js";
import { Sortable } from "./sortable";
import "../tokens.css";
import "../styles.css";
import "../buttons.css";
import "./u485.css";

const u484Fixture = () => {
  const external = new URLSearchParams(location.search).has("external");
  const dense = new URLSearchParams(location.search).has("dense");
  const [ids, order] = createSignal(dense ? Array.from({ length: 40 }, (_, index) => String(index + 1)) : ["A", "B", "C"]);
  const [commits, commit] = createSignal(0);
  const [actions, action] = createSignal(0);
  const [dropped, drop] = createSignal<string[]>([]);
  const [files, file] = createSignal<string[]>([]);

  return <>
    <Sortable.Root ids={ids()} external={external ? {
      target: (_id, x, y) => {
        const element = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-outside]");

        return element ? { element } : null;
      },
      onDropOutside: (id, target) => drop([...dropped(), `${id}>${target.element.dataset.outside}`]),
    } : undefined} onOrderChange={(next) => { order(next); commit(commits() + 1); }}
      aria-label="Ordered rows" class="sortable-fixture" onDragOver={(event) => event.preventDefault()}
      onDrop={(event) => { event.preventDefault(); file([...files(), ...Array.from(event.dataTransfer?.files ?? [], (entry) => entry.name)]); }}>
      <For each={ids()}>{(id, index) => <Sortable.Item id={id} index={index()}>{(parts) =>
        <div ref={parts.ref} data-id={id} class="sortable-row" data-dragging={parts.isDragging()}>
          <button ref={parts.handleRef} data-button="quiet" data-reads="text" aria-label={`Move ${id}`} onClick={() => action(actions() + 1)}>Move {id}</button>
          <span>{id}</span>
          <button data-button="quiet" aria-label={`Inspect ${id}`} onClick={() => action(actions() + 1)}>Inspect</button>
        </div>
      }</Sortable.Item>}</For>
    </Sortable.Root>
    {external && <div data-outside="Rail" class="sortable-row">Rail</div>}
    <output aria-label="Dropped">{dropped().join(",")}</output>
    <output aria-label="Committed order">{ids().join(",")}</output>
    <output aria-label="Commits">{commits()}</output>
    <output aria-label="Actions">{actions()}</output>
    <output aria-label="Files">{files().join(",")}</output>
  </>;
};

render(u484Fixture, document.getElementById("root")!);
