import { describe, expect, it } from "vitest";
import styles from "../styles.css?inline";

const NARROW = /@media \(max-width: 1101px\)\s*{([\s\S]*?)}\s*}/;

const effective = (css: string, selector: string, property: string) =>
  [...css.matchAll(new RegExp(`(?:^|\\n)\\s*${selector}\\s*{([^}]*)}`, "g"))]
    .flatMap(([, body]) => [...(body ?? "").matchAll(new RegExp(`${property}:\\s*([^;]+);`, "g"))].map((m) => m[1]))
    .at(-1);

const wide = styles.replace(NARROW, "");

const narrow = `${wide}\n${NARROW.exec(styles)?.[0].replace(/^[^{]*{/, "").replace(/}\s*$/, "") ?? ""}`;

describe("u23 column widths", () => {
  it("u23_wide_the_rail_and_shelf_are_clamped_and_the_centre_keeps_a_floor_of_690px", () => {
    expect(effective(wide, "\\.columns", "grid-template-columns")).toBe("clamp(252px, 20%, 320px) minmax(690px, 1fr) clamp(160px, 14%, 280px)");
  });
});

describe("u24 narrow window", () => {
  it("u24_under_1102px_the_rail_stays_252px_and_the_centre_shrinks_instead_of_keeping_its_floor", () => {
    expect(effective(narrow, "\\.columns", "grid-template-columns")).toBe("252px 1fr");
  });

  it("u24_under_1102px_the_shelf_sits_under_the_rail_and_the_centre_spans_both_rows", () => {
    expect(effective(narrow, "\\.rail", "grid-area")).toBe("1 / 1");
    expect(effective(narrow, "\\.shelf", "grid-area")).toBe("2 / 1");
    expect(effective(narrow, "\\.centre", "grid-area")).toBe("1 / 2 / span 2");
  });

  it("u24_the_shelf_is_never_hidden", () => {
    expect(effective(narrow, "\\.shelf", "display")).toBeUndefined();
  });

  it("u24_the_drawer_leaves_the_rail_visible", () => {
    expect(effective(narrow, "\\.drawer", "width")).toBe("min(560px, 70%, calc(100% - 252px))");
  });
});
