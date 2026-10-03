import { describe, expect, it } from "vitest";

// The categories slice 2 of U130 moves out of a stylesheet (docs/design-system.md, "Tokens"; D1).
const LOOK_PROPERTIES = [
  "color",
  "background",
  "background-color",
  "border-color",
  "box-shadow",
  "border-radius",
  "font-size",
  "transition-duration",
  "animation-duration",
] as const;

const isTokenRead = (value: string): boolean => /^var\(--[\w-]+\)$/.test(value.trim());

// Anchored on the char before the name so "background-color: …" never also answers for "color".
const lookLiteralsIn = (css: string): string[] => {
  const found: string[] = [];

  for (const property of LOOK_PROPERTIES) {
    const pattern = new RegExp(`(?:^|[\\s;{])${property}:\\s*([^;]+);`, "g");

    for (const match of css.matchAll(pattern)) {
      const value = match[1]!.trim();

      if (!isTokenRead(value)) found.push(`${property}: ${value}`);
    }
  }

  return found;
};

describe("u130 terminal/styles.css literal scan", () => {
  // A literal copy, not LOOK_PROPERTIES itself: dropping a property from the scanner must fail this loop, not
  // shrink it.
  const propertiesToWatch = [
    "color",
    "background",
    "background-color",
    "border-color",
    "box-shadow",
    "border-radius",
    "font-size",
    "transition-duration",
    "animation-duration",
  ];

  it("u130_terminal_literal_scan_flags_a_literal_on_every_look_property_but_not_a_token_read", () => {
    for (const property of propertiesToWatch) {
      expect(lookLiteralsIn(`${property}: #ffffff;\n`), property).toEqual([`${property}: #ffffff`]);
      expect(lookLiteralsIn(`${property}: var(--grey);\n`), property).toEqual([]);
    }
  });

  it("u130_terminal_literal_scan_does_not_match_a_longer_property_name", () => {
    expect(lookLiteralsIn("background-color: #fff;\n")).toEqual(["background-color: #fff"]);
  });

  it("u130_terminal_literal_scan_rejects_a_value_only_partly_a_token_read", () => {
    expect(lookLiteralsIn("box-shadow: 0 1px 2px var(--hairline);\n")).toEqual([
      "box-shadow: 0 1px 2px var(--hairline)",
    ]);
  });

  it("u130_terminal_literal_scan_ignores_a_spacing_property", () => {
    expect(lookLiteralsIn("margin-left: 1ch;\nbottom: 8px;\n")).toEqual([]);
  });

  it("u130_terminal_styles_css_has_no_look_literal", () => {
    const files = import.meta.glob<string>("./styles.css", { query: "?raw", import: "default", eager: true });
    const css = Object.values(files)[0];

    expect(css, "styles.css is read").toBeTruthy();
    expect(lookLiteralsIn(css!)).toEqual([]);
  });
});
