import { promptOf } from "./spawnPrompt";

/** U33's inline field: Enter spawns with its text (or none when blank), Esc closes and calls nothing. */
export const SpawnPromptField = (props: { onSubmit: (prompt: string | null) => void; onCancel: () => void }) => (
  <input
    aria-label="prompt"
    ref={(field) => queueMicrotask(() => field.focus())}
    onKeyDown={(key) => {
      if (key.key === "Enter") props.onSubmit(promptOf(key.currentTarget.value));
      else if (key.key === "Escape") props.onCancel();
    }}
  />
);
