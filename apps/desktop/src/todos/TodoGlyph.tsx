import type { Todo } from "@contracts/todo/Todo";
import { glyphOf } from "../ink/glyph";
import { kindOf } from "./todoView";

export const TodoGlyph = (props: { todo: Todo }) => {
  const glyph = () => glyphOf(kindOf(props.todo));

  return (
    <span data-tone={glyph().tone} style={{ color: `var(--${glyph().tone})` }}>
      {glyph().mark}
    </span>
  );
};
