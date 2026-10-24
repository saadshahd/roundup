import { captureFocus } from "../keys/focus";
import { copy } from "../copy";

const promptOf = (text: string): string | null => {
  const trimmed = text.trim();

  return trimmed === "" ? null : trimmed;
};

/** U2227: grows to its text; the CSS max-height caps it at 6 lines, then it scrolls. */
const fit = (field: HTMLTextAreaElement) => {
  field.style.height = "auto";
  field.style.height = `${field.scrollHeight}px`;
};

export const SpawnPromptField = (props: {
  value: string;
  selection: { start: number; end: number; direction: "forward" | "backward" | "none" } | null;
  onSelection: (selection: { start: number; end: number; direction: "forward" | "backward" | "none" }) => void;
  onInput: (text: string, selection: { start: number; end: number; direction: "forward" | "backward" | "none" }) => void;
  onSubmit: (prompt: string | null) => void;
  onCancel: () => void;
  ref: (field: HTMLTextAreaElement) => void;
}) => (
  <textarea
    aria-label={copy.task}
    placeholder={copy.taskHint}
    rows={1}
    class="spawn-prompt-field"
    value={props.value}
    ref={(field) => {
    const focus = captureFocus();
      queueMicrotask(() => {
        focus(() => field.focus());
        const selection = props.selection;
        field.setSelectionRange(selection?.start ?? field.value.length, selection?.end ?? field.value.length, selection?.direction ?? "none");
        fit(field);
      });
      props.ref(field);
    }}
    onInput={(typed) => {
      fit(typed.currentTarget);
      props.onInput(typed.currentTarget.value, { start: typed.currentTarget.selectionStart, end: typed.currentTarget.selectionEnd, direction: typed.currentTarget.selectionDirection });
    }}
    onSelect={(event) => { if (event.currentTarget.isConnected) props.onSelection({ start: event.currentTarget.selectionStart, end: event.currentTarget.selectionEnd, direction: event.currentTarget.selectionDirection }); }}
    onKeyDown={(press) => {
      if (press.key === "Enter" && !press.shiftKey && !press.isComposing) {
        press.preventDefault();
        props.onSubmit(promptOf(press.currentTarget.value));
      } else if (press.key === "Escape") props.onCancel();
    }}
  />
);
