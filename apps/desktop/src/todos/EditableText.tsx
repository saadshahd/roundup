import { createSignal, Show } from "solid-js";

/** Double-click edits; leaving the field commits once, and only when the text changed. */
export const EditableText = (props: {
  value: string;
  multiline: boolean;
  name: "title" | "body";
  onCommit: (text: string) => void;
}) => {
  const [editing, setEditing] = createSignal(false);

  const field = (element: HTMLInputElement | HTMLTextAreaElement) => {
    element.value = props.value;
    queueMicrotask(() => element.focus());

    // Removing a focused field can fire a second blur in a webview.
    let left = false;

    element.addEventListener("blur", () => {
      if (left) return;

      left = true;
      setEditing(false);

      if (element.value !== props.value) props.onCommit(element.value);
    });
  };

  return (
    <Show
      when={editing()}
      fallback={
        <p onDblClick={() => setEditing(true)} style={{ "white-space": "pre-wrap", "min-height": "1.4em" }}>
          <Show when={props.value !== ""} fallback={<span class="light">{`no ${props.name}`}</span>}>
            {props.value}
          </Show>
        </p>
      }
    >
      <Show
        when={props.multiline}
        fallback={
          <input
            ref={field}
            aria-label={props.name}
            onKeyDown={(key) => {
              if (key.key === "Enter") key.currentTarget.blur();
            }}
          />
        }
      >
        <textarea ref={field} aria-label={props.name} rows={6} />
      </Show>
    </Show>
  );
};
