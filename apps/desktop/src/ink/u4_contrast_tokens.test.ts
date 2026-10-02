import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const SRC = join(__dirname, "..");

/** Every rule block (`selector { body }`) in a stylesheet's text, so a check can see which selector owns a declaration. */
const rulesOf = (css: string): { selector: string; body: string }[] =>
  [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((rule) => ({ selector: rule[1]!.trim(), body: rule[2]! }));

/** A `color:` declaration, never `border-left-color:` or `background-color:`, which share the suffix but are not text colour. */
const TEXT_COLOUR = /(?:^|[\s;])color:\s*var\(--(light|lightest)\)/;

const isAllowedSelector = (selector: string): boolean => selector.includes(".glyph");

describe("u4 stylesheet text-colour allowlist", () => {
  it("u4_text_colour_rules_outside_glyph_tones_never_use_light_or_lightest", () => {
    for (const file of ["styles.css", "rail/styles.css"]) {
      const css = readFileSync(join(SRC, file), "utf8");

      const offenders = rulesOf(css)
        .filter((rule) => TEXT_COLOUR.test(rule.body))
        .filter((rule) => !isAllowedSelector(rule.selector))
        .map((rule) => rule.selector);

      expect(offenders, `${file}: ${offenders.join(", ")}`).toEqual([]);
    }
  });

  it("u4_the_light_inline_override_is_limited_to_the_pad_drawers_export_button", () => {
    const INLINE = /var\(--(?:light|lightest)\)/;
    const matches: string[] = [];

    const walk = (dir: string) => {
      for (const entry of readdirSync(dir, { withFileTypes: true })) {
        const path = join(dir, entry.name);

        if (entry.isDirectory()) {
          walk(path);
          continue;
        }

        if (!/\.tsx?$/.test(entry.name) || entry.name.endsWith(".test.ts") || entry.name.endsWith(".test.tsx")) continue;

        const text = readFileSync(path, "utf8");

        if (INLINE.test(text)) matches.push(path.slice(SRC.length + 1));
      }
    };

    walk(SRC);

    expect(matches).toEqual(["pads/PadDrawer.tsx"]);
  });
});
