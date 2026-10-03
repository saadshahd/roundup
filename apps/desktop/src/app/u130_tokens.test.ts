import { describe, expect, it } from "vitest";
import { inlineStyleCss, lookLiterals } from "../testing/lookLiterals";

const [table = ""] = Object.values(
  import.meta.glob<string>("../../../../docs/design-system.md", { query: "?raw", import: "default", eager: true }),
);

// The Tokens of the "Starting values" table, read from it so a Token added there fails here until tokens.css defines it.
const TOKENS = table
  .split("\n")
  .filter((row) => row.startsWith("| `--"))
  .flatMap((row) => row.split("|")[1]?.match(/--[\w-]+/g) ?? []);

const rows = table.split("\n").filter((row) => row.startsWith("| `--"));

// Token -> light value, from the table's Light column and the prose lines under it (type and space steps).
const expectedLight = new Map<string, string>();

for (const row of rows) {
  const cells = row.split("|");
  const names = cells[1]?.match(/--[\w-]+/g) ?? [];
  const values = [...(cells[2] ?? "").matchAll(/`([^`]+)`/g)].map((match) => match[1] ?? "");

  names.forEach((name, index) => expectedLight.set(name, values[index] ?? ""));
}

for (const [, name, value] of table.matchAll(/`(--font-[\w-]+)` is `([^`]+)`/g)) expectedLight.set(name ?? "", value ?? "");

for (const [, name, size] of table.matchAll(/`(--text-[\w-]+)` (\d+)/g)) expectedLight.set(name ?? "", `${size}px`);

const steps = /`--space-1` to `--space-6` are ([\d, ]+) px/.exec(table)?.[1]?.split(",") ?? [];

steps.forEach((step, index) => expectedLight.set(`--space-${index + 1}`, `${step.trim()}px`));

// Spelling only: case, spaces, a bare leading dot and trailing zeros differ between the table and CSS; hex colours compare as written.
const normalised = (value: string) =>
  value.startsWith("#") ? value.toLowerCase() : value.toLowerCase().replace(/\s+/g, "").replace(/\d*\.?\d+/g, (number) => String(Number(number)));

// Tokens whose light value is knowingly not the table's yet. Slice 2 of U130 moved `--text` to the table's
// `#1d1d1f` and deleted its entry; a later slice may add one for `--selected`, which the table's value fails D5 on.
const KNOWN_DIFFERENCES = new Map<string, string>();

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

  it("u130_the_expected_values_cover_the_table_and_the_type_and_space_steps", () => {
    const named = [...expectedLight.keys()];

    expect(TOKENS.length, "the table was read").toBeGreaterThanOrEqual(14);
    expect(named).toEqual(expect.arrayContaining(["--ground", "--hairline", "--radius-drawer"]));
    expect(named).toEqual(expect.arrayContaining([...TOKENS, "--font-ui", "--font-mono", "--text-title", "--space-1", "--space-6"]));
    expect([...expectedLight.values()].filter((value) => value === "")).toEqual([]);
  });

  it("u130_every_token_has_the_tables_light_value_except_the_named_differences", () => {
    const css = Object.values(import.meta.glob<string>("../tokens.css", { query: "?raw", import: "default", eager: true }))[0] ?? "";

    const wrong = [...expectedLight].filter(
      ([name, value]) => normalised(valueOf(css, name) ?? "") !== normalised(KNOWN_DIFFERENCES.get(name) ?? value),
    );

    expect(wrong, "Tokens whose light value is not the table's").toEqual([]);

    for (const [name, difference] of KNOWN_DIFFERENCES)
      expect(normalised(difference), `the named difference for ${name} is real, or the entry goes`).not.toBe(
        normalised(expectedLight.get(name) ?? ""),
      );
  });

  it("u130_main_and_the_harness_import_tokens_css_before_styles_css", () => {
    const entries = import.meta.glob<string>(["../main.tsx", "../testing/harness.tsx"], { query: "?raw", import: "default", eager: true });

    const positions = Object.values(entries).map((code) => [code.search(/import "\.{1,2}\/tokens\.css"/), code.search(/import "\.{1,2}\/styles\.css"/)]);

    expect(positions).toHaveLength(2);
    expect(positions.map(([tokens = -1, styles = -1]) => tokens >= 0 && tokens < styles)).toEqual([true, true]);
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

  it("u130_styles_css_and_terminal_styles_css_have_no_look_literal_outside_a_var_read", () => {
    const sheets = import.meta.glob<string>(["../styles.css", "../terminal/styles.css"], { query: "?raw", import: "default", eager: true });

    expect(Object.keys(sheets).sort()).toEqual(["../styles.css", "../terminal/styles.css"]);
    expect(Object.entries(sheets).flatMap(([path, css]) => lookLiterals(css).map((literal) => `${path}: ${literal}`))).toEqual([]);
  });

  it("u130_inline_styles_in_drawer_pads_and_todos_have_no_look_literal_outside_a_var_read", () => {
    const sources = import.meta.glob<string>(["../drawer/**/*.{ts,tsx}", "../pads/**/*.{ts,tsx}", "../todos/**/*.{ts,tsx}", "!../**/*.test.{ts,tsx}"], {
      query: "?raw",
      import: "default",
      eager: true,
    });

    expect(Object.keys(sources)).toEqual(expect.arrayContaining(["../drawer/DrawerHost.tsx", "../pads/PadDrawer.tsx", "../todos/Todos.tsx"]));
    expect(Object.keys(sources).filter((path) => /\.test\./.test(path)), "test files are not scanned").toEqual([]);
    expect(Object.entries(sources).flatMap(([path, source]) => lookLiterals(inlineStyleCss(source)).map((literal) => `${path}: ${literal}`))).toEqual([]);
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
