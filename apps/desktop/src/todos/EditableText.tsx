import { createSignal, Show } from "solid-js";

type Editing = { from: string; text: string };

/**
 * Double-click edits; leaving the field commits once, and only when the text differs from what the field started
 * with, so another Actor's change made meanwhile is never written back. A failed commit reopens the field with
 * the typed text.
 */
export const EditableText = (props: {
  value: string;
  multiline: boolean;
  name: "title" | "body";
  /** Resolves with the failure message, or `null` when the commit went through. */
  commit: (text: string) => Promise<string | null>;
}) => {
  const [editing, setEditing] = createSignal<Editing | null>(null);

  const startWith = (text: string) => setEditing({ from: props.value, text });

  const field = (started: Editing) => (element: HTMLInputElement | HTMLTextAreaElement) => {
    element.value = started.text;
    queueMicrotask(() => element.focus());

    // Removing a focused field can fire a second blur in a webview.
    let left = false;

    element.addEventListener("blur", () => {
      if (left) return;

      left = true;
      setEditing(null);

      if (element.value === started.from) return;

      void props.commit(element.value).then((failure) => {
        if (failure !== null) startWith(element.value);
      });
    });
  };

  return (
    <Show
      when={editing()}
      fallback={
        <p onDblClick={() => startWith(props.value)} style={{ "white-space": "pre-wrap", "min-height": "1.4em" }}>
          <Show when={props.value !== ""} fallback={<span class="light">{`no ${props.name}`}</span>}>
            {props.value}
          </Show>
        </p>
      }
      keyed
    >
      {(started) => (
        <Show
          when={props.multiline}
          fallback={
            <input
              ref={field(started)}
              aria-label={props.name}
              onKeyDown={(key) => {
                if (key.key === "Enter") key.currentTarget.blur();
              }}
            />
          }
        >
          <textarea ref={field(started)} aria-label={props.name} rows={6} />
        </Show>
      )}
    </Show>
  );
};
