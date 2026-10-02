const promptOf = (text: string): string | null => {
  const trimmed = text.trim();

  return trimmed === "" ? null : trimmed;
};

export const SpawnPromptField = (props: {
  onSubmit: (prompt: string | null) => void;
  onCancel: () => void;
  ref?: (field: HTMLInputElement) => void;
}) => (
  <input
    aria-label="prompt"
    ref={(field) => {
      queueMicrotask(() => field.focus());
      props.ref?.(field);
    }}
    onKeyDown={(key) => {
      if (key.key === "Enter") props.onSubmit(promptOf(key.currentTarget.value));
      else if (key.key === "Escape") props.onCancel();
    }}
  />
);
