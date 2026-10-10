import { splitProps } from "solid-js";
import type { JSX } from "solid-js";
import "../buttons.css";

/** The five kinds of docs/design-system.md, "Click targets": named by `data-button` and drawn by `buttons.css`. */
type ButtonKind = "primary" | "quiet" | "add" | "row-action" | "icon";

type ButtonProps = Omit<JSX.ButtonHTMLAttributes<HTMLButtonElement>, "disabled" | "data-button" | "onClick"> & {
  kind: ButtonKind;
  /** The button cannot act: it keeps focus and `aria-disabled`, never `disabled` (U171); its `title` can say why. */
  unavailable?: boolean;
  onClick?: (click: MouseEvent) => void;
};

export const Button = (props: ButtonProps) => {
  const [own, rest] = splitProps(props, ["kind", "unavailable", "onClick"]);

  return (
    <button
      type="button"
      {...rest}
      data-button={own.kind}
      aria-disabled={own.unavailable ? true : undefined}
      onClick={(click) => {
        if (own.unavailable) return click.preventDefault();

        own.onClick?.(click);
      }}
    />
  );
};
