import { describe, expect, it } from "vitest";


/** Every rule block (`selector { body }`) in a stylesheet's text, so a check can see which selector owns a declaration. */
const rulesOf = (css: string): { selector: string; body: string }[] =>
  [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((rule) => ({ selector: rule[1]!.trim(), body: rule[2]! }));

/** A `color:` declaration, never `border-left-color:` or `background-color:`, which share the suffix but are not text colour. */
const TEXT_COLOUR = /(?:^|[\s;])color:\s*var\(--(light|lightest)\)/;

const isAllowedSelector = (selector: string): boolean => selector.includes(".glyph");

describe("u4 stylesheet text-colour allowlist", () => {
  it("u4_text_colour_rules_outside_glyph_tones_never_use_light_or_lightest", () => {
    const sheets = import.meta.glob<string>(["../styles.css", "../rail/styles.css"], {
      query: "?raw",
      import: "default",
      eager: true,
    });

    expect(Object.keys(sheets).sort()).toEqual(["../rail/styles.css", "../styles.css"]);

    for (const [file, css] of Object.entries(sheets)) {
      const offenders = rulesOf(css)
        .filter((rule) => TEXT_COLOUR.test(rule.body))
        .filter((rule) => !isAllowedSelector(rule.selector))
        .map((rule) => rule.selector);

      expect(offenders, `${file}: ${offenders.join(", ")}`).toEqual([]);
    }
  });

  it("u4_the_light_inline_override_is_limited_to_the_pad_drawers_export_button", () => {
    const INLINE = /var\(--(?:light|lightest)\)/;
    const sources = import.meta.glob<string>(["../**/*.{ts,tsx}", "!../**/*.test.{ts,tsx}"], {
      query: "?raw",
      import: "default",
      eager: true,
    });
    const matches = Object.entries(sources)
      .filter(([, text]) => INLINE.test(text))
      .map(([path]) => path.slice("../".length));

    expect(matches).toEqual(["pads/PadDrawer.tsx"]);
  });
});
