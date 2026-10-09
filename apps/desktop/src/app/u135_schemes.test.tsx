import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { runChecks } from "../testing/checks";
import { contrastRatio, resolveToken } from "../testing/contrast";
import { rootTokensOf, underSettings } from "../testing/media";
import type { Setting } from "../testing/media";
import { seedApp } from "../testing/seeds";
import tokenCss from "../tokens.css?inline";
import appCss from "../styles.css?inline";
import inkCss from "../ink/styles.css?inline";
import padsCss from "../pads/styles.css?inline";
import railCss from "../rail/styles.css?inline";
import terminalCss from "../terminal/styles.css?inline";
import rowButtonCss from "../todos/rowButton.styles.css?inline";
import chipCss from "../rail/attentionChip.styles.css?inline";

const [table = ""] = Object.values(import.meta.glob<string>("../../../../docs/design-system.md", { query: "?raw", import: "default", eager: true }));

const SHEETS = [tokenCss, appCss, inkCss, padsCss, railCss, terminalCss, rowButtonCss, chipCss].join("\n");

const cssFor = (settings: Setting[]) => underSettings(SHEETS, settings);

/** Token -> dark value, from the Dark column of the table's colour rows. */
const darkColumn = new Map(
  table
    .split("\n")
    .filter((row) => row.startsWith("| `--") && row.split("|").length > 4)
    .flatMap((row) => {
      const cells = row.split("|");
      const names = cells[1]!.match(/--[\w-]+/g) ?? [];
      const dark = [...cells[3]!.matchAll(/`([^`]+)`/g)].map((one) => one[1]!);

      return dark.length === names.length ? names.map((name, index) => [name, dark[index]!] as const) : [];
    }),
);

const spelled = (value: string) => value.toLowerCase().replace(/\s+/g, "").replace(/\d*\.?\d+/g, (number) => String(Number(number)));

let sheet: HTMLStyleElement | undefined;

let matchMedia: typeof window.matchMedia | undefined;

afterEach(() => {
  sheet?.remove();

  if (matchMedia) window.matchMedia = matchMedia;
  matchMedia = undefined;
  cleanup();
});

const mount = async (settings: Setting[]) => {
  sheet = document.head.appendChild(document.createElement("style"));
  sheet.textContent = cssFor(settings);
  matchMedia = window.matchMedia;

  const feature = { dark: "prefers-color-scheme: dark", "more-contrast": "prefers-contrast: more", "reduced-transparency": "prefers-reduced-transparency: reduce", "reduced-motion": "prefers-reduced-motion: reduce" };
  window.matchMedia = (query) => ({
    matches: settings.some((setting) => query.includes(feature[setting])),
    media: query,
    onchange: null,
    addListener() {},
    removeListener() {},
    addEventListener() {},
    removeEventListener() {},
    dispatchEvent: () => false,
  });

  const controls = seedApp("agents-10", Date.now());
  render(() => <App app={controls.app} reducedMotion={() => settings.includes("reduced-motion")} clock={Date.now} />);
  await screen.findAllByRole("treeitem");
};

describe("u135 schemes", () => {
  it("u135_the_table_gives_a_dark_value_for_the_colours_and_the_shadow", () => {
    expect([...darkColumn.keys()]).toEqual(expect.arrayContaining(["--ground", "--sunken", "--hover", "--selected", "--text", "--grey", "--accent", "--amber", "--red", "--hairline", "--shadow-drawer"]));
  });

  it("u135_in_dark_every_token_takes_its_dark_value", () => {
    const dark = rootTokensOf(cssFor(["dark"]));

    for (const [name, value] of darkColumn) expect(spelled(dark.get(name.slice(2)) ?? ""), name).toBe(spelled(value));
  });

  it("u135_in_light_no_token_takes_a_dark_value", () => {
    const light = rootTokensOf(cssFor([]));

    expect(spelled(light.get("ground") ?? "")).toBe("#ffffff");
    expect(cssFor([])).toMatch(/:root\s*\{\s*color-scheme:\s*light dark;/);
  });

  it("u135_more_contrast_makes_grey_text_the_text_colour_and_every_hairline_40_percent", () => {
    const both: Setting[][] = [["more-contrast"], ["more-contrast", "dark"]];

    for (const settings of both) {
      const tokens = rootTokensOf(cssFor(settings));

      expect(resolveToken("var(--grey)", tokens), settings.join()).toBe(resolveToken("var(--text)", tokens));
      expect(tokens.get("hairline"), settings.join()).toMatch(/^rgba\(\s*\d+,\s*\d+,\s*\d+,\s*0?\.4\s*\)$/);
    }
  });

  it("u135_reduced_transparency_removes_every_backdrop_filter", async () => {
    await mount(["reduced-transparency"]);
    const rules = [...(sheet?.sheet?.cssRules ?? [])].filter((rule): rule is CSSStyleRule => "selectorText" in rule);
    const none = rules.filter((rule) => rule.style.getPropertyValue("backdrop-filter") === "none");

    expect(none.map((rule) => rule.selectorText.replace(/\s+/g, ""))).toContain("*,*::before,*::after");
    expect(cssFor([])).not.toMatch(/backdrop-filter:\s*(?!none)/);
  });

  it("u135_reduced_motion_makes_every_duration_but_the_instant_changes_0s", async () => {
    await mount(["reduced-motion"]);

    expect(runChecks().D9.status).toBe("pass");

    for (const el of document.body.querySelectorAll("*")) {
      const style = getComputedStyle(el);

      for (const duration of [style.transitionDuration, style.animationDuration]) expect(duration.split(",").every((one) => one.trim() === "" || one.trim() === "auto" || /^0m?s$/.test(one.trim())), `${el.className} ${duration}`).toBe(true);
    }
  });

  const each: Setting[][] = [[], ["dark"], ["more-contrast"], ["dark", "more-contrast"], ["reduced-transparency"], ["reduced-motion"]];

  for (const settings of each) {
    it(`u135_${settings.join("_") || "light"}_keeps_text_at_4_5_and_glyph_tones_at_3_and_the_other_checks`, async () => {
      await mount(settings);
      const tokens = rootTokensOf(cssFor(settings));
      const checks = runChecks();

      for (const id of ["D1", "D2", "D3", "D4", "D5", "D8", "D9"] as const) expect(checks[id], `${id} ${settings.join()}`).toMatchObject({ status: "pass" });

      const ground = resolveToken("var(--ground)", tokens);
      const sunken = resolveToken("var(--sunken)", tokens);

      for (const tone of ["text", "grey"]) for (const base of [ground, sunken]) expect(contrastRatio(resolveToken(`var(--${tone})`, tokens), base), `${tone} on ${base}`).toBeGreaterThanOrEqual(4.5);

      for (const tone of ["accent", "amber", "red"]) for (const base of [ground, sunken]) expect(contrastRatio(resolveToken(`var(--${tone})`, tokens), base), `${tone} on ${base}`).toBeGreaterThanOrEqual(tone === "accent" ? 4.5 : 3);
    });
  }

  it("u135_d10_holds_at_700_by_800", async () => {
    await mount(["dark"]);
    Object.assign(window, { innerWidth: 700, innerHeight: 800 });
    const d10 = runChecks().D10;

    expect(d10.value).toMatchObject({ viewport: "700x800", scheme: "dark" });
    expect(d10.status).toBe("pass");
  });
});
