import { copy } from "../copy";
import { createMemo, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import type { Accessor, JSX } from "solid-js";
import { IconButton } from "../ui/IconButton";
import { Tabs } from "../ui/Tabs";
import { ScopeControl } from "./ScopeControl";
import { useShelfScope } from "../todos/shelfScope";
import { ShelfActions } from "./BulkDelete";
import { Icon } from "../ink/Icon";
import { SectionHead } from "../ink/SectionHead";
import { Pads } from "../pads/Pads";
import { shownPads } from "../pads/shownPads";
import { useConnectedProject } from "../state/connectedProject";
import { Todos } from "../todos/Todos";
import { shownTodos } from "../todos/shownTodos";
import { useTodoList } from "../todos/todoList";
import { usePadList } from "../rail/pads/padList";
import { SHELF_TABS } from "./frame";
import type { ShelfTab } from "./frame";
import "./styles.css";

const LABELS: Record<ShelfTab, string> = { todos: copy.todos, pads: copy.pads };

/** U370: the one button that hides and shows the Shelf; it sits in the Shelf's tablist line while open and at the header's trailing edge while closed. */
export const ShelfToggle = (props: { open: boolean; onToggle: () => void }) => (
  <IconButton
    class="shelf-toggle"
    aria-label={props.open ? copy.hideShelf : copy.showShelf}
    aria-expanded={props.open}
    onClick={() => props.onToggle()}
  >
    <Icon name={props.open ? "right" : "left"} />
  </IconButton>
);

/** U371: a scroll shadow shows on an edge only where content is clipped there. */
const ScrollShadow = (props: { children: JSX.Element }) => {
  const [top, setTop] = createSignal(false);
  const [bottom, setBottom] = createSignal(false);
  let area: HTMLDivElement | undefined;

  const measure = () => {
    if (!area) return;

    setTop(area.scrollTop > 0);
    setBottom(area.scrollTop + area.clientHeight < area.scrollHeight - 1);
  };

  onMount(() => {
    measure();

    if (typeof ResizeObserver === "undefined" || !area) return;

    const observer = new ResizeObserver(measure);

    observer.observe(area);

    for (const child of area.children) observer.observe(child);

    onCleanup(() => observer.disconnect());
  });

  return (
    <div class="shelf-scroll" data-clipped-top={top()} data-clipped-bottom={bottom()}>
      <div class="shelf-area" ref={(element) => (area = element)} onScroll={measure}>
        {props.children}
      </div>
    </div>
  );
};

/** U373: a tablist of `Todos` and `Pads` with a quiet count each, and the open tab's list as the only panel. */
export const Shelf = (props: { open: Accessor<boolean>; tab: Accessor<ShelfTab>; onTab: (tab: ShelfTab) => void; onHide: () => void }) => {
  const [actions, setActions] = createSignal<HTMLDivElement>();
  const connected = useConnectedProject();
  const scope = useShelfScope(connected);
  const todos = useTodoList();
  const pads = usePadList();
  const openTodos = shownTodos(connected, todos);
  const shown = shownPads(connected, pads);
  const counts = createMemo<Record<ShelfTab, number>>(() => ({ todos: openTodos().length, pads: shown().length }));

  return (
    <Tabs.Root
      id="shelf"
      value={props.tab()}
      onValueChange={({ value }) => { if (value === "todos" || value === "pads") props.onTab(value); }}
      lazyMount
      unmountOnExit
      style={{ display: "contents" }}
      ids={{ trigger: (tab) => `shelf-tab-${tab}`, content: (tab) => `shelf-panel-${tab}` }}
    >
      <div class="shelf-bar">
        <Tabs.List class="shelf-tabs" aria-label={copy.shelfLabel}>
          <For each={SHELF_TABS}>
            {(tab) => (
              <Tabs.Trigger value={tab} class="shelf-tab" onClick={() => props.onTab(tab)}>
                <SectionHead icon={tab === "todos" ? "check" : "order"} label={LABELS[tab]} />
                <Show when={counts()[tab] > 0}>
                  <span class="shelf-count">{counts()[tab]}</span>
                </Show>
              </Tabs.Trigger>
            )}
          </For>
        </Tabs.List>
        <Show when={scope.workstream()}>
          <ScopeControl showAll={scope.showAll()} onChange={scope.setShowAll} />
        </Show>
        <ShelfToggle open onToggle={props.onHide} />
      </div>
      <ShelfActions.Provider value={{ mount: actions, open: props.open }}>
      <ScrollShadow>
        <For each={SHELF_TABS}>
          {(tab) => (
            <Tabs.Content value={tab} class="shelf-panel">
              <Show when={tab === "todos"} fallback={<Pads />}>
                <Todos />
              </Show>
            </Tabs.Content>
          )}
        </For>
      </ScrollShadow>
      <div ref={setActions} />
      </ShelfActions.Provider>
    </Tabs.Root>
  );
};
