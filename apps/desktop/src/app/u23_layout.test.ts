import { describe, expect, it } from "vitest";
import styles from "../styles.css?inline";

const declared = (selector: string, property: string, css = styles) =>
  new RegExp(`${selector}\\s*{[^}]*?${property}:\\s*([^;]+);`).exec(css)?.[1];

describe("u23 column widths", () => {
  it("u23_the_columns_give_the_rail_and_shelf_clamped_widths_and_the_terminal_pane_a_floor_of_80_cells", () => {
    expect(declared("\\.columns", "grid-template-columns")).toBe("clamp(252px, 20%, 320px) minmax(680px, 1fr) clamp(160px, 14%, 280px)");
  });

  it("u24_under_1100px_the_shelf_is_hidden_and_the_columns_are_the_rail_and_the_terminal_pane", () => {
    const narrow = /@media \(max-width: 1099px\)\s*{([\s\S]*?)}\s*}/.exec(styles)?.[1] ?? "";

    expect(declared("\\.columns", "grid-template-columns", narrow)).toBe("clamp(252px, 20%, 320px) 1fr");
    expect(declared("\\.shelf", "display", narrow)).toBe("none");
  });
});
