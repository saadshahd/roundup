import { describe, expect, it } from "vitest";

const [table = ""] = Object.values(
  import.meta.glob<string>("../../../../docs/design-system.md", { query: "?raw", import: "default", eager: true }),
);

// The Tokens of the "Starting values" table, read from it so a Token added there fails here until tokens.css defines it.
const TOKENS = table
  .split("\n")
  .filter((row) => row.startsWith("| `--"))
  .flatMap((row) => row.split("|")[1]?.match(/--[\w-]+/g) ?? []);

function valueOf(section: string, token: string): string | undefined {
  return section.match(new RegExp(`(?:^|[\\s;{])${token}:\\s*([^;\\s][^;]*);`))?.[1];
}

describe("u130 tokens", () => {
  it("u130_token_lookup_does_not_match_a_longer_property_name", () => {
    const css = "--row--hover: red;\n--hover: blue;\n";
    expect(valueOf(css, "--hover")).toBe("blue");
  });

  it("u130_an_empty_token_value_does_not_count", () => {
    expect(valueOf("--sunken: ;\n", "--sunken")).toBeUndefined();
  });

  it("u130_every_design_system_token_has_a_light_value", () => {
    const files = import.meta.glob<string>("../tokens.css", {
      query: "?raw",
      import: "default",
      eager: true,
    });

    const css = Object.values(files)[0];
    expect(css, "tokens.css is read").toBeTruthy();

    const withoutLight = TOKENS.filter((token) => !valueOf(css!, token));

    expect(withoutLight, "tokens missing a light value").toEqual([]);
  });

  // Slice 3 of U130 gives every Token a dark value and turns this into a test.
  it.todo("u130_every_token_has_a_dark_value");

  it("u130_styles_css_does_not_shadow_a_shared_token", () => {
    const files = import.meta.glob<string>("../styles.css", {
      query: "?raw",
      import: "default",
      eager: true,
    });

    const css = Object.values(files)[0];
    expect(css, "styles.css is read").toBeTruthy();

    const shadowed = TOKENS.filter((token) => valueOf(css!, token));

    expect(shadowed, "tokens.css's values must not be shadowed in styles.css").toEqual([]);
  });

  // Slice 1 keeps the App light by setting no scheme at all; slice 3 deletes this test with the dark block.
  it("u130_no_stylesheet_sets_a_dark_scheme_before_slice_3", () => {
    const sheets = import.meta.glob<string>("../**/*.css", { query: "?raw", import: "default", eager: true });

    const dark = Object.entries(sheets)
      .filter(([, css]) => /prefers-color-scheme|color-scheme/.test(css))
      .map(([path]) => path);

    expect(Object.keys(sheets).length, "stylesheets are read").toBeGreaterThan(1);
    expect(dark).toEqual([]);
  });
});
