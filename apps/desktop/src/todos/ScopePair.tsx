import { createUniqueId } from "solid-js";

/** U159: the `todos` header's native radio pair, `all` and `this room`; the radios are the words' own inputs, so the keyboard is the browser's. */
export const ScopePair = (props: { thisRoom: boolean; onChange: (thisRoom: boolean) => void }) => {
  const name = `todos-scope-${createUniqueId()}`;

  return (
    <span role="radiogroup" aria-label="todos shown" class="scope-pair">
      {[
        { label: "all", value: false },
        { label: "this room", value: true },
      ].map((option) => (
        <label class="word scope-word">
          <input
            type="radio"
            name={name}
            checked={props.thisRoom === option.value}
            onChange={() => props.onChange(option.value)}
          />
          <span data-on={props.thisRoom === option.value ? "" : undefined}>{option.label}</span>
        </label>
      ))}
    </span>
  );
};
