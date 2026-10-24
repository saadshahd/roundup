import { captureFocus } from "../keys/focus";
import "./Combobox.css";
import "./Select.css";
import { Portal } from "solid-js/web";
import { Popover } from "./Popover";
import ChevronDown from "lucide-solid/icons/chevron-down";
import { copy } from "../copy";
import { Combobox as ArkCombobox, createListCollection } from "@ark-ui/solid/combobox";
import { createMemo, createSignal, createUniqueId, For, onCleanup, type JSX } from "solid-js";

type ComboboxItem = { value: string; label: string };

/** An always-open, filter-as-you-type list. The caller filters `items` and controls the field; Ark owns the highlight and `↑` `↓` (no wrap); a resting or moving pointer never moves the highlight. `onSelect` hears the highlighted value on `⏎`. */
const Root = (props: {
  items: readonly ComboboxItem[];
  highlightedValue: string | null;
  onHighlightChange: (value: string | null) => void;
  inputValue: string;
  /** Native selection/caret access; changing the query goes through `inputValue`. Focus only on explicit opening. */
  inputRef?: (input: HTMLInputElement) => void;
  initialFocus?: ReturnType<typeof captureFocus>;
  onDismiss?: () => void;
  focusScope?: () => HTMLElement | undefined;
  onInput: (text: string) => void;
  onSelect: (value: string) => void;
  label: string;
  placeholder: string;
  empty: string;
  group?: (item: ComboboxItem) => string | undefined;
  row: (item: ComboboxItem) => JSX.Element;
}) => {
  const dismiss = props.onDismiss ?? Popover.dismiss();
  const contains = (target: EventTarget | null, input: HTMLElement) => target instanceof Node && (props.focusScope?.() ?? input.closest(".app-combobox"))?.contains(target) === true;
  const collection = createMemo(() => createListCollection({ items: [...props.items], itemToValue: (item) => item.value, itemToString: (item) => item.label, groupBy: (item) => props.group?.(item) ?? "" }));

  // Ark highlights the row under the pointer; the highlight follows typing and the arrows alone, so a pointer event's own highlight change is dropped.
  let pointerEvent = false;

  const notePointer = (): void => {
    pointerEvent = true;
    setTimeout(() => { pointerEvent = false; });
  };

  return <ArkCombobox.Root class="app-combobox" collection={collection()} open lazyMount={false} inputBehavior="none" selectionBehavior="clear" loopFocus={false} autoFocus={false}
    onFocusOutside={event => { if (!(event.detail.target instanceof Node && props.focusScope?.()?.contains(event.detail.target))) dismiss?.(); }}
    inputValue={props.inputValue} value={[]} highlightedValue={props.highlightedValue ?? undefined}
    onHighlightChange={(details) => { if (!pointerEvent) props.onHighlightChange(details.highlightedValue); }}
    onInputValueChange={(details) => props.onInput(details.inputValue)}
    onValueChange={(details) => {
      const [value] = details.value;

      if (value !== undefined) props.onSelect(value);
    }}>
    <ArkCombobox.Label class="app-combobox-label">{props.label}</ArkCombobox.Label>
    <ArkCombobox.Control><ArkCombobox.Input ref={input => {
      const focus = props.initialFocus ?? Popover.initialFocus() ?? captureFocus();
      const signal = focus.signal(target => contains(target, input));
      const cancel = () => dismiss?.();
      signal.addEventListener("abort", cancel, { once: true });
      onCleanup(() => signal.removeEventListener("abort", cancel));

      if (signal.aborted) queueMicrotask(() => { if (input.isConnected) cancel(); });
      props.inputRef?.(input);
      queueMicrotask(() => focus(() => { if (input.isConnected) input.focus(); }));
    }} class="app-combobox-input" placeholder={props.placeholder} /></ArkCombobox.Control>
    <ArkCombobox.Content class="app-combobox-list" on:pointermove={{ handleEvent: notePointer, capture: true }} on:pointerleave={{ handleEvent: notePointer, capture: true }}>
      <For each={collection().group()} fallback={<p class="app-combobox-empty">{props.empty}</p>}>
        {([group, items]) => group ? <ArkCombobox.ItemGroup>
          <ArkCombobox.ItemGroupLabel class="app-combobox-group-label">{group}</ArkCombobox.ItemGroupLabel>
          <For each={items}>{(item) => <ArkCombobox.Item item={item} class="app-combobox-item">{props.row(item)}</ArkCombobox.Item>}</For>
        </ArkCombobox.ItemGroup> : <For each={items}>{(item) => <ArkCombobox.Item item={item} class="app-combobox-item">{props.row(item)}</ArkCombobox.Item>}</For>}
      </For>
    </ArkCombobox.Content>
  </ArkCombobox.Root>;
};


/** A filtered choice anchored to its trigger. Ark owns opening, dismissal and focus return. */
const Picker = (props: { label: string; value: string; savedLabel?: string; items: readonly ComboboxItem[]; onSelect: (value: string) => void }) => {
  const caption = createUniqueId();
  const valueId = createUniqueId();
  const [open, setOpen] = createSignal(false);
  const [query, setQuery] = createSignal("");
  const [highlight, setHighlight] = createSignal<string | null>(null);
  let input: HTMLInputElement | undefined;
  const items = () => props.items.filter(item => item.label.toLocaleLowerCase().includes(query().toLocaleLowerCase()));

  return <Popover.Root open={open()} onOpenChange={({ open }) => { setOpen(open); setQuery(""); setHighlight(null); }} autoFocus closeOnEscape closeOnInteractOutside initialFocusEl={() => input ?? null} positioning={{ placement: "bottom-start", strategy: "fixed" }}>
    <Popover.Trigger class="app-select-trigger" aria-labelledby={`${caption} ${valueId}`}>
      <span id={caption} class="app-combobox-label">{props.label}</span>
      <span id={valueId}>{props.items.find(item => item.value === props.value)?.label ?? props.savedLabel ?? props.label}</span>
      <ChevronDown aria-hidden="true" class="app-select-mark" size={16} />
    </Popover.Trigger>
    <Portal><Popover.Positioner><Popover.Content class="choice-picker" aria-label={props.label} onKeyDown={event => { if (event.key === "Escape") event.stopPropagation(); }}>
      <Root items={items()} highlightedValue={highlight()} onHighlightChange={setHighlight} inputValue={query()} inputRef={element => { input = element; }} onInput={setQuery}
        onSelect={value => { props.onSelect(value); setOpen(false); }} label={props.label} placeholder={props.label} empty={copy.noMatch} row={item => item.label} />
    </Popover.Content></Popover.Positioner></Portal>
  </Popover.Root>;
};

export const Combobox = { Root, Picker };
