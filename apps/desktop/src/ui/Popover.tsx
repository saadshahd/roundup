import { captureFocus } from "../keys/focus";
import { Popover as ArkPopover } from "@ark-ui/solid/popover";
import { createComputed, createContext, createSignal, on, useContext } from "solid-js";
import { dialogOwned, floatingControl } from "./Dialog";
import "./Popover.css";

const Owner = createContext<{ setElement: (element: HTMLElement) => void; focus: () => ReturnType<typeof captureFocus>; close: () => void }>();

/** A popover that never takes the keyboard on opening and closes on outside interaction or a modal opening. */
export const Popover = {
  ...ArkPopover,
  initialFocus: () => useContext(Owner)?.focus(),
  dismiss: () => useContext(Owner)?.close,
  Root: (props: ArkPopover.RootProps & { initialFocus?: ReturnType<typeof captureFocus> | undefined }) => {
    const [element, setElement] = createSignal<HTMLElement>();
    const control = floatingControl(props, element);
    let initialFocus = captureFocus();
    let finalFocus = captureFocus();
    let returnFocus = true;

    createComputed(on(control.open, open => {
      if (open) { initialFocus = props.initialFocus ?? captureFocus(true); returnFocus = true; }
      else finalFocus = captureFocus();
    }));

    return <Owner.Provider value={{ setElement, focus: () => initialFocus, close: () => { returnFocus = false; control.change({ open: false }); } }}><ArkPopover.Root lazyMount unmountOnExit autoFocus={false} modal={false} closeOnEscape={false} {...props} open={control.open()} onOpenChange={control.change}
      initialFocusEl={() => {
        let target: HTMLElement | null | false = false;
        initialFocus(() => { target = props.initialFocusEl?.() ?? null; });

        return target;
      }} finalFocusEl={() => returnFocus ? finalFocus.target(() => props.finalFocusEl?.() ?? element()) : (document.activeElement instanceof HTMLElement ? document.activeElement : document.body)} /></Owner.Provider>;
  },
  Trigger: (props: ArkPopover.TriggerProps) => {
    const owner = useContext(Owner);

    return <ArkPopover.Trigger {...props} ref={(element: HTMLButtonElement) => {
      owner?.setElement(element);

      // SAFETY: callers supply callback refs; Solid compiles element bindings into callbacks.
      (props.ref as ((element: HTMLButtonElement) => void) | undefined)?.(element);
    }} />;
  },
  Anchor: (props: ArkPopover.AnchorProps) => {
    const owner = useContext(Owner);

    return <ArkPopover.Anchor {...props} ref={(element: HTMLDivElement) => {
      owner?.setElement(element);

      // SAFETY: callers supply callback refs; Solid compiles element bindings into callbacks.
      (props.ref as ((element: HTMLDivElement) => void) | undefined)?.(element);
    }} />;
  },
  Content: (props: ArkPopover.ContentProps) => <ArkPopover.Content {...props} data-dialog-owned={dialogOwned() || undefined} class={`app-popover ${props.class ?? ""}`} />,
};
