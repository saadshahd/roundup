import { createUniqueId } from "solid-js";

/** U159: the `todos` header's native radio pair, `all` and `this one`; the radios are the words' own inputs, so the keyboard is the browser's. */
export const WorkstreamChoice = (props: { thisWorkstream: boolean; onChange: (thisWorkstream: boolean) => void }) => {
  const name = `todos-shown-${createUniqueId()}`;

  return (
    <span role="radiogroup" aria-label="todos shown" class="shown-choice">
      {[
        { label: "all", value: false },
        { label: "this one", value: true },
      ].map((option) => (
        <label class="word shown-word">
          <input
            type="radio"
            name={name}
            checked={props.thisWorkstream === option.value}
            onChange={() => props.onChange(option.value)}
          />
          <span>{option.label}</span>
        </label>
      ))}
    </span>
  );
};
