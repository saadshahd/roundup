import { captureFocus, queueFocus } from "../keys/focus";
import { createSignal } from "solid-js";
import type { Accessor } from "solid-js";

/** U3814: the open center page and its original return destination, retained when changing pages. */
export type SettingsViewState = {
  open: Accessor<boolean>;
  page: Accessor<"project-settings" | "import-work">;
  show(opener: HTMLElement, page?: "project-settings" | "import-work", restore?: () => void): void;
  /** Closes the view; `refocus` false leaves focus where the user put it. */
  close(refocus?: boolean): void;
};

export const createSettingsView = (): SettingsViewState => {
  const [open, setOpen] = createSignal(false);
  const [page, setPage] = createSignal<"project-settings" | "import-work">("project-settings");
  let restore: (() => void) | undefined;
  let opener: HTMLElement | undefined;

  return {
    open, page,
    show: (from, next = "project-settings", returning) => {
      captureFocus(true);

      if (!open()) { opener = from; restore = returning; }

      setPage(next); setOpen(true);
    },
    close: (refocus = true) => {
      setOpen(false);

      if (refocus) {
        restore?.();

        const focus = () => {
          const target = opener?.isConnected ? opener : document.querySelector<HTMLElement>(".todo-project-root");
          target?.focus();
        };

        focus();
        queueFocus(() => { if (document.activeElement === document.body) focus(); });
      }
    },
  };
};
