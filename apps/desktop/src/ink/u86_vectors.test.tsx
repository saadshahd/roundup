import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, expect, it } from "vitest";
import { KindGlyph } from "./KindGlyph";
import { ErrorLine } from "./ErrorLine";

afterEach(cleanup);

it.each(["error", "needs-you", "blocked", "working", "idle", "done"] as const)(
  "u86_%s_has_a_named_vector_without_font_text",
  (kind) => {
    const { container } = render(() => <KindGlyph kind={kind} />);
    const mark = screen.getByRole("img", { name: kind });
    expect(mark.querySelector("svg")).not.toBeNull();
    expect(container.textContent).toBe("");
  },
);

it("u86_error_text_has_one_decorative_vector", () => {
  const { container } = render(() => <ErrorLine message="could not open" />);
  expect(screen.getByText("could not open").querySelector('svg[aria-hidden="true"]')).not.toBeNull();
  expect(container.textContent).toBe("could not open");
});

it("u86_kind_marks_do_not_depend_on_hue", () => {
  const kinds = ["error", "needs-you", "blocked", "working", "idle", "done"] as const;
  const { container } = render(() => <>{kinds.map((kind) => <KindGlyph kind={kind} />)}</>);
  const marks = [...container.querySelectorAll("svg")].map((svg) => [svg.innerHTML, svg.getAttribute("fill")].join(":"));
  expect(new Set(marks).size).toBe(kinds.length);
});
