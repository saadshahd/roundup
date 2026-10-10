import { cleanup, fireEvent, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { seedApp } from "../testing/seeds";
import tokenCss from "../tokens.css?inline";

const [doc = ""] = Object.values(import.meta.glob<string>("../../../../docs/design-system.md", { query: "?raw", import: "default", eager: true }));

const sheets = import.meta.glob<string>("../**/*.css", { query: "?raw", import: "default", eager: true });

const KINDS = ["primary", "quiet", "add", "row-action", "icon"];

afterEach(cleanup);

const mount = async (seed: "agents-10" | "tree-40" | "decisions") => {
  window.matchMedia ??= (query) => ({ matches: false, media: query, onchange: null, addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {}, dispatchEvent: () => false });
  const controls = seedApp(seed, Date.now());

  render(() => <App app={controls.app} reducedMotion={() => true} clock={Date.now} />);
  await screen.findAllByRole("treeitem");
};

const clickTargets = () => [...document.querySelectorAll("button, [role=button]")].filter((el) => !el.closest(".milkdown") && !el.hasAttribute("data-row"));

describe("u170 five button kinds, each defined once", () => {
  it("u170_the_click_targets_section_defines_exactly_five_kinds", () => {
    const section = /## Click targets([\s\S]*?)\n## /.exec(doc)?.[1] ?? "";
    const kinds = [...section.matchAll(/^\| `(primary|quiet|add|row-action|icon)` \|/gm)].map((row) => row[1]);

    expect(kinds).toEqual(KINDS);
  });

  it("u170_every_button_of_the_rail_shelf_and_centre_names_one_of_the_five_kinds", async () => {
    await mount("agents-10");
    fireEvent.click(screen.getAllByRole("treeitem")[0]!);

    const targets = clickTargets();

    expect(targets.length).toBeGreaterThan(5);
    expect(targets.filter((el) => !KINDS.includes(el.getAttribute("data-button") ?? "")).map((el) => el.outerHTML.slice(0, 80))).toEqual([]);
  });

  it("u170_a_row_takes_no_data_button_and_a_button_in_a_row_is_one_of_the_five", async () => {
    await mount("tree-40");
    const rows = screen.getAllByRole("treeitem");

    expect(rows.filter((row) => row.hasAttribute("data-button"))).toEqual([]);

    for (const inside of rows.flatMap((row) => [...row.querySelectorAll("button")])) expect(KINDS, inside.outerHTML.slice(0, 80)).toContain(inside.getAttribute("data-button"));
  });

  it("u170_only_buttons_css_sets_a_background_colour_radius_or_padding_on_a_button", () => {
    const rules = (css: string) => [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((rule) => ({ selector: rule[1]!.replace(/\/\*[\s\S]*?\*\//g, "").trim(), body: rule[2]! }));

    const offenders = Object.entries(sheets).flatMap(([file, css]) =>
      file.endsWith("/buttons.css")
        ? []
        : rules(css)
            .filter((rule) => /(^|[\s>+~,])button\b/.test(rule.selector) && /(^|[\s;])(background(-color)?|color|border-radius|padding[\w-]*)\s*:/.test(rule.body))
            .map((rule) => `${file}: ${rule.selector}`),
    );

    expect(offenders).toEqual([]);
  });

  it("u170_the_pressed_and_disabled_tokens_have_a_light_and_a_dark_value", () => {
    const dark = /@media \(prefers-color-scheme: dark\)\s*\{([\s\S]*?)\n\}/.exec(tokenCss)?.[1] ?? "";

    for (const token of ["--pressed", "--disabled"]) {
      expect(doc, token).toContain(`\`${token}\``);
      expect(tokenCss, token).toMatch(new RegExp(`:root\\s*\\{[\\s\\S]*${token}:`));
      expect(dark, token).toContain(`${token}:`);
    }
  });
});
