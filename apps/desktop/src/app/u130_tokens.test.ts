import { describe, expect, it } from "vitest";

// Every Token docs/design-system.md's "Starting values" table lists; U130 fails any left without both schemes.
const TOKENS = [
  "--ground",
  "--sunken",
  "--hover",
  "--selected",
  "--text",
  "--grey",
  "--accent",
  "--amber",
  "--red",
  "--hairline",
  "--shadow-drawer",
  "--radius-row",
  "--radius-control",
  "--radius-drawer",
] as const;

function valueOf(section: string, token: string): string | undefined {
  return section.match(new RegExp(`(?:^|[\\s;{])${token}:\\s*([^;]+);`))?.[1];
}

describe("u130 tokens", () => {
  it("u130_token_lookup_does_not_match_a_longer_property_name", () => {
    const css = "--row--hover: red;\n--hover: blue;\n";
    expect(valueOf(css, "--hover")).toBe("blue");
  });

  it("u130_every_design_system_token_has_a_light_and_a_dark_value", () => {
    const files = import.meta.glob<string>("../tokens.css", {
      query: "?raw",
      import: "default",
      eager: true,
    });

    const css = Object.values(files)[0];
    expect(css, "tokens.css is read").toBeTruthy();

    const darkIndex = css!.indexOf("prefers-color-scheme: dark");
    expect(darkIndex, "a dark scheme block exists").toBeGreaterThan(-1);

    const light = css!.slice(0, darkIndex);
    const dark = css!.slice(darkIndex);

    const withoutLight = TOKENS.filter((token) => !valueOf(light, token));
    const withoutDark = TOKENS.filter((token) => !valueOf(dark, token));

    expect(withoutLight, "tokens missing a light value").toEqual([]);
    expect(withoutDark, "tokens missing a dark value").toEqual([]);
  });

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
});
