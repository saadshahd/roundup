import { captureFocus } from "../keys/focus";
import * as select from "@zag-js/select";
import { normalizeProps, useMachine } from "@zag-js/solid";
import { getInitialFocus } from "@zag-js/dom-query";
import { useLocaleContext } from "@ark-ui/solid/locale";
import { useEnvironmentContext } from "@ark-ui/solid/environment";
import { useFieldContext } from "@ark-ui/solid/field";
import "./Select.css";
import { floatingControl } from "./Dialog";
import { Select as ArkSelect, createListCollection } from "@ark-ui/solid/select";
import { createMemo, createSignal, createUniqueId, For, onCleanup, type JSX } from "solid-js";
import ChevronDown from "lucide-solid/icons/chevron-down";
import { Portal } from "solid-js/web";

export type SelectChoice = { value: string; label: string };

/** A labelled single-choice list over Ark's Select; Escape in an open list closes only that list. */
export const Select = (props: {
  label: string;
  choices: SelectChoice[];
  value: string;
  onValueChange: (value: string) => void;
}): JSX.Element => {
  const collection = createMemo(() => createListCollection({ items: props.choices, itemToValue: (choice) => choice.value, itemToString: (choice) => choice.label }));
  const [root, setRoot] = createSignal<HTMLDivElement>();
  const [list, setList] = createSignal<HTMLDivElement>();
  const control = floatingControl({}, root);
  const closing = new WeakSet<Event>();
  let trigger: HTMLButtonElement | undefined;
  let initialFocus = captureFocus();
  let returnFocus = captureFocus();
  const requestFocus = () => { initialFocus = captureFocus(true); returnFocus = captureFocus(); };

  const id = createUniqueId();
  const locale = useLocaleContext();
  const environment = useEnvironmentContext();
  const field = useFieldContext();

  // Ark's useSelect fixes the machine and exposes no final-focus hook. Keep Zag's transitions,
  // guards and effects; only the two scheduled focus actions carry the App's permission.
  const machine: typeof select.machine = {
    ...select.machine,
    implementations: {
      ...select.machine.implementations,
      actions: {
        ...select.machine.implementations?.actions,
        setInitialFocus: () => {
          const focus = initialFocus;
          requestAnimationFrame(() => focus(() => {
            const target = getInitialFocus({ root: list() ?? null });

            if (target) target.focus({ preventScroll: true });
          }));
        },
        focusTriggerEl: parameters => {
          if (!select.machine.implementations?.guards?.restoreFocus?.(parameters)) return;
          const focus = returnFocus;
          requestAnimationFrame(() => focus(() => { if (trigger?.isConnected) trigger.focus({ preventScroll: true }); }));
        },
      },
    },
  };

  const service = useMachine(machine, () => ({
    id, collection: collection(), value: [props.value],
    ids: { label: field?.().ids.label, hiddenSelect: field?.().ids.control },
    disabled: field?.().disabled ?? false, readOnly: field?.().readOnly ?? false, invalid: field?.().invalid ?? false, required: field?.().required ?? false,
    dir: locale().dir, getRootNode: environment().getRootNode,
    onValueChange: details => props.onValueChange(details.value[0] ?? props.value),
    open: control.open(), onOpenChange: control.change, positioning: { strategy: "fixed" as const, sameWidth: true },
  }));

  const api = createMemo(() => select.connect(service, normalizeProps));

  // Ark closes the list before an Escape reaches the form, so mark the Escape that found it open first.
  const mark = (event: KeyboardEvent) => {
    const target = event.target;

    if (event.key === "Escape" && control.open() && target instanceof Node && (root()?.contains(target) || list()?.contains(target))) closing.add(event);
  };

  document.addEventListener("keydown", mark, true);
  onCleanup(() => document.removeEventListener("keydown", mark, true));

  return <div class="app-select" ref={setRoot} onKeyDown={(event) => { if (closing.has(event)) event.stopPropagation(); }}>
    <ArkSelect.RootProvider value={api} lazyMount unmountOnExit>
      <ArkSelect.Label class="app-select-label">{props.label}</ArkSelect.Label>
      <ArkSelect.Control>
        <ArkSelect.Trigger ref={element => {
          trigger = element;
          const key = (event: KeyboardEvent) => { if (["Enter", " ", "ArrowUp", "ArrowDown"].includes(event.key)) requestFocus(); };

          element.addEventListener("click", requestFocus, true);
          element.addEventListener("keydown", key, true);
          onCleanup(() => { element.removeEventListener("click", requestFocus, true); element.removeEventListener("keydown", key, true); });
        }} class="app-select-trigger" aria-label={props.label}>
          <ArkSelect.ValueText />
          <ChevronDown aria-hidden="true" class="app-select-mark" size={16} />
        </ArkSelect.Trigger>
      </ArkSelect.Control>
      <Portal>
        <ArkSelect.Positioner>
          <ArkSelect.Content class="app-select-list" ref={element => {
            setList(element);
            const returning = () => { returnFocus = captureFocus(); };

            element.addEventListener("keydown", returning, true);
            element.addEventListener("click", returning, true);
            onCleanup(() => { element.removeEventListener("keydown", returning, true); element.removeEventListener("click", returning, true); });
          }}>
            <For each={props.choices}>{(choice) => <ArkSelect.Item item={choice} class="app-select-item">
              <ArkSelect.ItemText>{choice.label}</ArkSelect.ItemText>
            </ArkSelect.Item>}</For>
          </ArkSelect.Content>
        </ArkSelect.Positioner>
      </Portal>
      <ArkSelect.HiddenSelect />
    </ArkSelect.RootProvider>
  </div>;
};
