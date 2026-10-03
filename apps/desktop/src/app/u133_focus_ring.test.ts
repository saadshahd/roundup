import { describe, expect, it } from "vitest";

const sheets = import.meta.glob<string>("../**/*.css", { query: "?raw", import: "default", eager: true });

const uncommented = (css: string) => css.replace(/\/\*[\s\S]*?\*\//g, "");

const sheetNamed = (name: string) => uncommented(Object.entries(sheets).find(([path]) => path.endsWith(name))?.[1] ?? "");

const rules = (css: string) => [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map(([, selector = "", body = ""]) => ({ selector: selector.trim(), body }));

describe("u133 one focus ring rule", () => {
  it("u133_styles_css_has_one_bare_focus_visible_rule_that_draws_the_accent_ring", () => {
    const bare = rules(sheetNamed("/src/styles.css")).filter((rule) => rule.selector === ":focus-visible");

    expect(bare).toHaveLength(1);
    expect(bare[0]?.body).toMatch(/outline:\s*2px\s+solid\s+var\(--accent\)/);
    expect(bare[0]?.body).toMatch(/outline-offset:\s*1px/);
  });

  it("u133_no_stylesheet_sets_another_outline_colour_or_removes_the_outline", () => {
    const offences = Object.entries(sheets).flatMap(([path, css]) =>
      rules(uncommented(css))
        .filter((rule) => rule.selector !== ":focus-visible")
        .flatMap((rule) => [...rule.body.matchAll(/(outline(?:-color)?)\s*:\s*([^;]+)/g)].map(([, property = "", value = ""]) => `${path} ${rule.selector} { ${property}: ${value.trim()} }`)),
    );

    expect(offences).toEqual([]);
  });
});
