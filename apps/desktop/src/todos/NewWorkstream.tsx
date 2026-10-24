import { captureFocus } from "../keys/focus";
import { Portal } from "solid-js/web";
import { createEffect, on, createMemo, createSignal, Show } from "solid-js";
import type { Accessor } from "solid-js";
import type { RailNode } from "@contracts/agent/RailNode";
import type { StartChoice } from "@contracts/workstream/StartChoice";
import { copy } from "../copy";
import { ReservedLine } from "../ink/ReservedLine";
import { useConnectedProject } from "../state/connectedProject";
import { registerKeys } from "../keys/registry";
import { Dialog } from "../ui/Dialog";
import { RadioGroup } from "../ui/RadioGroup";
import { Combobox } from "../ui/Combobox";
import { Tip } from "../ui/Tip";
import { Popover } from "../ui/Popover";
import { Icon } from "../ink/Icon";
import { failureOf } from "./failureOf";
import { useTodoList } from "./todoList";
import "./blockerMenu.styles.css";

type Start = StartChoice["kind"];

const startChoices = (): readonly { value: Start; label: string }[] => [{ value: "now", label: copy.startNow }, { value: "queued", label: copy.startWhenSlotFrees }, { value: "afterTodo", label: copy.startAfterTodo }];

const startChoiceOf = (start: Start, todo: number | null): StartChoice => start === "afterTodo" ? { kind: start, todo: todo! } : { kind: start };

/** The trigger shows the Todo chosen, or a prompt; it opens the shared filterable Combobox anchored under it, and Esc closes only the list. */
const WaitFor = (props: { todos: readonly { id: number; title: string }[]; value: string; onPick: (value: string) => void }) => {
  const [open, setOpen] = createSignal(false);
  const [query, setQuery] = createSignal("");
  const [highlight, setHighlight] = createSignal<string | null>(null);

  const shown = createMemo(() => {
    const needle = query().trim().toLowerCase();

    return props.todos.filter(todo => `#${todo.id} ${todo.title}`.toLowerCase().includes(needle)).map(todo => ({ value: String(todo.id), label: todo.title }));
  });

  const highlighted = () => shown().some(item => item.value === highlight()) ? highlight() : shown()[0]?.value ?? null;
  const chosen = () => props.todos.find(todo => String(todo.id) === props.value);
  let button: HTMLButtonElement | undefined;
  let panel: HTMLDivElement | undefined;
  const close = () => { setOpen(false); setQuery(""); setHighlight(null); };

  registerKeys({ kind: "layer", element: () => panel, active: open, modal: false }, [{
    chords: ["Escape"], run: () => { close(); button?.focus(); },
  }]);

  return <Popover.Root closeOnInteractOutside={false} open={open()} onOpenChange={({ open }) => { if (!open) close(); }} positioning={{ placement: "bottom-start", strategy: "fixed" }}>
    <Popover.Trigger asChild={attributes => <button {...attributes()} ref={element => { button = element; }} type="button" data-button="quiet" class="app-dialog-pick" onClick={() => { if (open()) close(); else setOpen(true); }}>
      <span>{chosen() ? `#${chosen()!.id} ${chosen()!.title}` : copy.chooseTodo}</span><Icon name="down" />
    </button>} />
    <Popover.Positioner><Popover.Content ref={element => { panel = element; }} class="todo-blocker-menu" onKeyDown={event => { if (event.key === "Escape") { event.stopPropagation(); close(); } }}>
      <Combobox.Root items={shown()} inputValue={query()} onInput={setQuery} highlightedValue={highlighted()} onHighlightChange={setHighlight}
        onSelect={value => { props.onPick(value); close(); }}
        label={copy.startAfterTodoLabel} placeholder={copy.blockerPlaceholder} empty={copy.noMatch}
        row={item => <><span class="todo-blocker-title">{item.label}</span><span class="mono light">#{item.value}</span></>} />
    </Popover.Content></Popover.Positioner>
  </Popover.Root>;
};

/** U3743: the dialog the owner picker's `New Workstream…` opens for Todo `target`. `Create` is the one `todo.startWorkstream` call; `done` runs after it with the Workstream made, or with `null` for Esc or `Cancel`, which call nothing. Esc or `Cancel` call nothing. */
export const NewWorkstream = (props: { target: Accessor<number | null>; done: (id: number, made: RailNode | null, focus?: ReturnType<typeof captureFocus>) => void; returnTo: (id: number) => HTMLElement | null }) => {
  const connected = useConnectedProject();
  const todos = useTodoList();

  const retained = createMemo<number | null>(previous => props.target() ?? previous, null);

  return <Show when={retained()}>{id => {
    const [name, setName] = createSignal(todos.byId().get(id())?.title ?? "");
    const [start, setStart] = createSignal<Start>(startChoices()[1]!.value);
    const [after, setAfter] = createSignal<string>("");
    const [failure, setFailure] = createSignal<string | null>(null);
    const [pending, setPending] = createSignal(false);
    createEffect(on(props.target, target => {
      if (target === null) return;
      setName(todos.byId().get(target)?.title ?? "");
      setStart(startChoices()[1]!.value);
      setAfter("");
      setFailure(null);
    }));
    const others = createMemo(() => todos.all().filter(todo => !todo.done && todo.id !== id()));
    const chosen = () => start() === "afterTodo" ? (others().find(todo => String(todo.id) === after())?.id ?? null) : null;
    const ready = () => name().trim() !== "" && (start() !== "afterTodo" || chosen() !== null) && !pending();

    const createBlocked = () => name().trim() === "" ? copy.createNeedsName : start() === "afterTodo" && chosen() === null ? copy.createNeedsTodo : undefined;

    const create = async () => {
      if (!ready()) return;

      const focus = captureFocus();
      setPending(true);
      setFailure(null);

      let made!: RailNode;
      const message = await failureOf(async () => { made = await connected.app.rpc("todo.startWorkstream", { id: id(), name: name().trim(), start: startChoiceOf(start(), chosen()) }); });

      setPending(false);

      if (message !== null) return setFailure(message);

      props.done(id(), made, focus);
    };

    return <Dialog.Root open={props.target() !== null} onOpenChange={({ open }) => { if (!open) props.done(id(), null); }} finalFocusEl={() => props.returnTo(id())}>
      <Portal><Dialog.Positioner>
        <Dialog.Content class="new-workstream" aria-label={copy.newWorkstreamTitle}>
          <form class="app-dialog-form" onSubmit={event => { event.preventDefault(); void create(); }}>
            <Dialog.Title>{copy.newWorkstreamTitle}</Dialog.Title>
            <label class="app-dialog-name">
              <span class="app-dialog-label">{copy.workstreamNameLabel}</span>
              <textarea rows={1} value={name()} onInput={event => setName(event.currentTarget.value)}
                onKeyDown={event => { if (event.key === "Enter" && !event.isComposing) { event.preventDefault(); event.currentTarget.form?.requestSubmit(); } }} />
            </label>
            <RadioGroup label={copy.startChoiceLabel} value={start()} onValueChange={setStart}
              choices={startChoices()} />
            <Show when={start() === "afterTodo"}>
              <div class="app-dialog-name">
                <span class="app-dialog-label">{copy.startAfterTodoLabel}</span>
                <WaitFor todos={others()} value={after()} onPick={setAfter} />
              </div>
            </Show>
            <ReservedLine message={failure()} />
            <div class="app-dialog-actions">
              <Dialog.CloseTrigger data-button="quiet">{copy.cancel}</Dialog.CloseTrigger>
              <Tip disabled={ready()} text={createBlocked()}>{trigger =>
                <button {...trigger()} type="submit" data-button="primary" aria-disabled={!ready()}>{copy.createWorkstream}</button>
              }</Tip>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Positioner></Portal>
    </Dialog.Root>;
  }}</Show>;
};
