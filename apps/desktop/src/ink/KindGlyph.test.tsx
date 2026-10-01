import { cleanup, render } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import type { Kind } from "@contracts/Kind";
import { KindGlyph } from "./KindGlyph";

const markOf = (kind: Kind) => {
  const { container } = render(() => <KindGlyph kind={kind} />);

  return container.querySelector("span");
};

afterEach(cleanup);

describe("u4 ink", () => {
  it.each([
    ["needs-you", "●", "amber"],
    ["error", "✕", "red"],
    ["blocked", "⏸", "grey"],
    ["working", "○", "light"],
    ["idle", "·", "light"],
    ["done", "✓", "lightest"],
  ] as const)("u4_the_glyph_component_draws_%s_as_%s_in_%s", (kind, mark, tone) => {
    const span = markOf(kind);

    expect([span?.textContent, span?.dataset["tone"]]).toEqual([mark, tone]);
  });

  it.each([
    ["needs-you", true],
    ["error", true],
    ["blocked", false],
    ["working", false],
    ["idle", false],
    ["done", false],
  ] as const)("u4_the_glyph_component_for_%s_carries_ink_%s", (kind, ink) => {
    expect(markOf(kind)?.classList.contains("ink")).toBe(ink);
  });
});
