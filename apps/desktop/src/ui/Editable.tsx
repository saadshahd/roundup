import { captureFocus } from "../keys/focus";
import { Editable as ArkEditable } from "@ark-ui/solid/editable";

export const Editable = {
  ...ArkEditable,
  Root: (props: ArkEditable.RootProps) => {
    let finalFocus = captureFocus();

    return <ArkEditable.Root selectOnFocus autoResize {...props} onEditChange={details => {
      if (!details.edit) finalFocus = captureFocus();
      props.onEditChange?.(details);
    }} finalFocusEl={props.finalFocusEl ? () => finalFocus.target(props.finalFocusEl!) : undefined} class={`app-editable ${props.class ?? ""}`} />;
  },
};
