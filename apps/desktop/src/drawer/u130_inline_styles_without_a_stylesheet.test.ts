import { describe, expect, it } from "vitest";

/**
 * The body of every `style={{ ... }}` object and every exported `JSX.CSSProperties` object in one file's
 * text — the two shapes a component in `drawer/`, `pads/` or `todos/` uses for its own look (`pads/nameLine.ts`'s
 * `markedLine`, `cutName`, `wholeWord`).
 */
const styleTextOf = (source: string): string =>
  [...source.matchAll(/style=\{\{([\s\S]*?)\}\}/g), ...source.matchAll(/:\s*JSX\.CSSProperties\s*=\s*\{([^}]*)\}/g)]
    .map((match) => match[1])
    .join("\n");

/**
 * A colour, font size, radius or box-shadow written directly instead of `var(--token)` (docs/design-system.md,
 * "Tokens"). Duration is not scanned here: the Drawer's slide duration (`drawer.ts`'s `DRAWER_SLIDE_MS`) is a bare
 * number assigned to a named constant, not text inside a style block, and the Starting-values table names no
 * duration Token yet, so U130 slice 2 reports it instead of inventing one.
 */
const LOOK_LITERAL =
  /#[0-9a-fA-F]{3,8}\b|\brgba?\([^)]*\)|(?:border-radius|font-size)"?\s*[:,]\s*"?\d+(?:\.\d+)?px\b|\bbox-shadow\b/;

const sourceFiles = () =>
  import.meta.glob<string>(["./**/*.{ts,tsx}", "../pads/**/*.{ts,tsx}", "../todos/**/*.{ts,tsx}"], {
    query: "?raw",
    import: "default",
    eager: true,
  });

describe("u130 inline styles in directories with no stylesheet", () => {
  it("u130_style_text_extracts_an_inline_style_objects_body", () => {
    expect(styleTextOf('<div style={{ color: "var(--text)" }} />')).toContain('color: "var(--text)"');
  });

  it("u130_style_text_extracts_a_shared_css_properties_constant", () => {
    expect(styleTextOf('export const wholeWord: JSX.CSSProperties = { "flex-shrink": "0" };')).toContain(
      "flex-shrink",
    );
  });

  it("u130_style_text_ignores_text_outside_a_style_or_css_properties_literal", () => {
    expect(styleTextOf('<button aria-label="make #120 yours">#120</button>')).toBe("");
  });

  it("u130_look_literal_matches_a_hex_colour", () => {
    expect(LOOK_LITERAL.test('color: "#1d1d1f"')).toBe(true);
  });

  it("u130_look_literal_matches_an_rgba_colour", () => {
    expect(LOOK_LITERAL.test('background: "rgba(0, 0, 0, 0.07)"')).toBe(true);
  });

  it("u130_look_literal_matches_a_pixel_radius", () => {
    expect(LOOK_LITERAL.test('"border-radius": "6px"')).toBe(true);
  });

  it("u130_look_literal_matches_a_pixel_font_size", () => {
    expect(LOOK_LITERAL.test('"font-size": "13px"')).toBe(true);
  });

  it("u130_look_literal_matches_a_box_shadow_declaration", () => {
    expect(LOOK_LITERAL.test('"box-shadow": "0 2px 4px black"')).toBe(true);
  });

  it("u130_look_literal_does_not_match_a_token_read_or_a_ch_unit", () => {
    expect(LOOK_LITERAL.test('color: "var(--grey)", "padding-left": "2ch"')).toBe(false);
  });

  it("u130_the_scan_reaches_drawer_pads_and_todos", () => {
    const paths = Object.keys(sourceFiles());

    expect(paths).toEqual(
      expect.arrayContaining(["./DrawerHost.tsx", "../pads/PadDrawer.tsx", "../todos/Todos.tsx"]),
    );
  });

  it("u130_drawer_pads_and_todos_hold_no_look_literal_in_a_style_block", () => {
    const offenders = Object.entries(sourceFiles())
      .map(([path, text]) => [path, styleTextOf(text)] as const)
      .filter(([, styleText]) => LOOK_LITERAL.test(styleText))
      .map(([path]) => path);

    expect(offenders).toEqual([]);
  });
});
