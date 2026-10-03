import { describe, expect, it } from "vitest";
import { inlineStyleCss, lookLiterals } from "./lookLiterals";

const literals = (css: string) => lookLiterals(`.x {\n  ${css}\n}`);

describe("u130 the look-literal scanner", () => {
  it("u130_scanner_flags_a_literal_for_each_look_category", () => {
    const cases: [string, string][] = [
      ["color: #1d1d1f;", "color: #1d1d1f"],
      ["background: #ffffff;", "background: #ffffff"],
      ["border-left-color: #c7c7cc;", "border-left-color: #c7c7cc"],
      ["color: red;", "color: red"],
      ["stroke: red;", "stroke: red"],
      ["border-left-color: red;", "border-left-color: red"],
      ["border: 1px solid hsl(0 0% 50%);", "border: 1px solid hsl(0 0% 50%)"],
      ["border: 1px solid #FFFFFF;", "border: 1px solid #FFFFFF"],
      ["outline: 2px solid red;", "outline: 2px solid red"],
      ["border-top: 1px solid black;", "border-top: 1px solid black"],
      ["text-shadow: 0 1px red;", "text-shadow: 0 1px red"],
      ["background-image: linear-gradient(red, blue);", "background-image: linear-gradient(red, blue)"],
      ["background: oklch(0.5 0.1 200);", "background: oklch(0.5 0.1 200)"],
      ["background: hwb(200 10% 10%);", "background: hwb(200 10% 10%)"],
      ["font: 1.1em Menlo;", "font: 1.1em Menlo"],
      ["transition-delay: 100ms;", "transition-delay: 100ms"],
      ["background: red;", "background: red"],
      ["font-size: large;", "font-size: large"],
      ["fill: hsl(0 0% 50%);", "fill: hsl(0 0% 50%)"],
      ["box-shadow: 0 8px 32px rgba(0, 0, 0, 0.14);", "box-shadow: 0 8px 32px rgba(0, 0, 0, 0.14)"],
      ["border-radius: 6px;", "border-radius: 6px"],
      ["border-top-left-radius: 6px;", "border-top-left-radius: 6px"],
      ["border-radius: 50%;", "border-radius: 50%"],
      ["font-size: 13px;", "font-size: 13px"],
      ["font-size: 0.8em;", "font-size: 0.8em"],
      ["font: 13px Menlo;", "font: 13px Menlo"],
      ["transition-duration: 120ms;", "transition-duration: 120ms"],
      ["animation-duration: 120ms;", "animation-duration: 120ms"],
      ["transition: opacity 150ms ease-out;", "transition: opacity 150ms ease-out"],
      ["animation: fade 0.15s;", "animation: fade 0.15s"],
    ];

    for (const [declaration, expected] of cases) expect(literals(declaration), declaration).toEqual([expected]);
  });

  it("u130_scanner_allows_a_var_read_for_each_category_including_the_drawer_shadow", () => {
    expect(
      literals(
        "color: var(--text);\n background: var(--ground);\n box-shadow: var(--shadow-drawer);\n border-radius: var(--radius-row);\n font-size: var(--text-body);\n transition: transform var(--duration-drawer) ease-out;",
      ),
    ).toEqual([]);
  });

  it("u130_scanner_allows_the_keywords_that_are_not_look_values", () => {
    expect(literals("color: inherit;\n background: transparent;\n box-shadow: none;\n border-radius: 0;\n transition: none;\n font-size: inherit;\n transition-duration: 0s;\n animation-duration: 0ms;")).toEqual([]);
  });

  it("u130_scanner_flags_a_value_only_partly_a_var_read", () => {
    expect(literals("background: #fff var(--ground);")).toEqual(["background: #fff var(--ground)"]);
    expect(literals("box-shadow: 0 1px 2px var(--hairline);")).toEqual(["box-shadow: 0 1px 2px var(--hairline)"]);
  });

  it("u130_scanner_reads_the_last_declaration_without_a_semicolon_and_compact_rules", () => {
    expect(lookLiterals(".a { color: red }")).toEqual(["color: red"]);
    expect(lookLiterals("color: red")).toEqual(["color: red"]);
    expect(lookLiterals(".a{COLOR:RED}")).toEqual(["COLOR: RED"]);
    expect(lookLiterals(".a{border-radius:4px;}")).toEqual(["border-radius: 4px"]);
  });

  it("u130_scanner_reads_an_indented_multi_line_rule_and_a_space_before_the_semicolon", () => {
    expect(lookLiterals(".a {\n  margin: 0;\n  color: #fff ;\n}\n.b {\n\tbackground: #000;\n}")).toEqual(["color: #fff", "background: #000"]);
    expect(literals("color: var(--text) ;")).toEqual([]);
  });

  it("u130_scanner_does_not_match_a_longer_property_name", () => {
    expect(literals("xbackground: red;\n --row--hover: red;")).toEqual([]);
  });

  it("u130_scanner_flags_a_custom_property_that_holds_a_colour_or_a_time_except_the_two_slice_3_deletes", () => {
    expect(literals("--light: #86868b;\n --lightest: #c7c7cc;")).toEqual([]);
    expect(literals("--row-band: #123;")).toEqual(["--row-band: #123"]);
    expect(literals("--slide: 180ms;")).toEqual(["--slide: 180ms"]);
    expect(literals("--x: oklch(0.5 0.1 200);")).toEqual(["--x: oklch(0.5 0.1 200)"]);
    expect(literals("--y: color-mix(in srgb, red, blue);")).toEqual(["--y: color-mix(in srgb, red, blue)"]);
    expect(literals("--space-3: 12px;\n --mono: ui-monospace, Menlo;")).toEqual([]);
  });

  it("u130_scanner_allows_a_border_and_an_outline_that_read_tokens", () => {
    expect(literals("background-image: linear-gradient(to right, var(--ground), var(--sunken));")).toEqual([]);
    expect(literals("border: 1px solid var(--hairline);\n outline: 2px solid var(--accent);\n text-decoration: underline;\n border-top: none;")).toEqual([]);
  });

  it("u130_scanner_ignores_spacing_comments_and_the_font_family", () => {
    expect(lookLiterals("/* color: red; */\n.a { margin-left: 1ch; bottom: 8px; font-family: var(--mono); }")).toEqual([]);
  });

  it("u130_inline_extractor_turns_a_style_object_and_a_css_properties_constant_into_css", () => {
    const source = '<div style={{ color: "var(--text)", "font-size": "13px" }} />\nexport const whole: JSX.CSSProperties = { "border-radius": "6px" };';

    expect(lookLiterals(inlineStyleCss(source))).toEqual(["font-size: 13px", "border-radius: 6px"]);
  });

  it("u130_inline_extractor_ignores_text_outside_a_style_block_and_allows_tokens_and_ch", () => {
    expect(inlineStyleCss('<button aria-label="make #120 yours">#120</button>')).toBe("");
    expect(lookLiterals(inlineStyleCss('<p style={{ color: "var(--grey)", "padding-left": "2ch", "box-shadow": "var(--shadow-drawer)" }} />'))).toEqual([]);
  });

  it("u130_inline_scan_reads_a_template_literal_a_ternary_and_a_single_quoted_colour", () => {
    const flagged = (pair: string) => lookLiterals(inlineStyleCss(`<i style={{ ${pair} }} />`));

    expect(flagged("transition: reduced() ? \"none\" : `transform ${MS}ms ${EASING}`")).toHaveLength(1);
    expect(flagged("transition: reduced() ? \"none\" : `transform var(--duration-drawer) ${EASING}`")).toEqual([]);
    expect(flagged('color: hot ? "red" : "var(--text)"')).toHaveLength(1);
    expect(flagged("transition: 'opacity var(--d) ease, transform 200ms ease'")).toHaveLength(1);
    expect(flagged("transition: `opacity var(--d) ease, transform 200ms ease`")).toHaveLength(1);
    expect(flagged("color: '#fff'")).toEqual(["color: #fff"]);
    expect(flagged("color: `#fff`")).toEqual(["color: #fff"]);
    expect(flagged('color: hot ? "var(--grey)" : "var(--text)"')).toHaveLength(1);
  });

  it("u130_inline_scan_flags_named_and_hsl_colours_and_em_sizes", () => {
    const flagged = (pair: string) => lookLiterals(inlineStyleCss(`<i style={{ ${pair} }} />`));

    expect(flagged('color: "white"')).toEqual(["color: white"]);
    expect(flagged('background: "hsl(0 0% 50%)"')).toEqual(["background: hsl(0 0% 50%)"]);
    expect(flagged('"font-size": "1.1rem"')).toEqual(["font-size: 1.1rem"]);
    expect(flagged('"border-radius": "50%"')).toEqual(["border-radius: 50%"]);
  });
});
