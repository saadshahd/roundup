import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { contrastRatio, resolveToken, tokensFrom } from "../testing/contrast";
import { driveStates, outlineRulesIn, styleIn } from "../testing/pointerStates";
import { seedApp } from "../testing/seeds";
import tokenCss from "../tokens.css?inline";
import appCss from "../styles.css?inline";
import inkCss from "../ink/styles.css?inline";
import padsCss from "../pads/styles.css?inline";
import railCss from "./styles.css?inline";
import terminalCss from "../terminal/styles.css?inline";
import rowButtonCss from "../todos/rowButton.styles.css?inline";
import chipCss from "./attentionChip.styles.css?inline";
import { SpawnPromptField } from "./SpawnPromptField";
import "./styles.css";

const tokens = tokensFrom(tokenCss);

const resolved = (value: string) => value.replace(/var\(--([\w-]+)\)/g, (_, name: string) => tokens.get(name) ?? name);

let undrive = () => {};

// main.tsx loads these two; the test mounts the App without it.
const shell = (() => {
  const sheet = document.createElement("style");
  sheet.textContent = `${tokenCss}\n${appCss}`;

  return sheet;
})();

afterEach(() => {
  undrive();
  undrive = () => {};

  shell.remove();
  cleanup();
});

const mount = async (seed: "tree-40" | "daemon-exits") => {
  document.head.prepend(shell);
  const controls = seedApp(seed, Date.now());
  render(() => <App app={controls.app} reducedMotion={() => true} clock={Date.now} />);
  await screen.findAllByRole("treeitem");

  return controls;
};

const CLICK_TARGET = "button, [role=treeitem], .spawn-prompt-field";

const targetsIn = (root: ParentNode) => [...root.querySelectorAll(CLICK_TARGET)].filter((el) => !el.hasAttribute("disabled") && el.getAttribute("aria-disabled") !== "true");

const px = (value: string) => Number.parseFloat(resolved(value));

/** The Rail, the Shelf, the header and the pane's pinned actions, then a Todo Drawer and a Pad Drawer open on top. */
const openEverything = async () => {
  await mount("tree-40");
  fireEvent.click((await screen.findAllByRole("button", { name: /#5/ }))[0]!);
  await screen.findByRole("complementary", { name: "drawer" });
  const drawer = screen.getByRole("complementary", { name: "drawer" });
  const section = { rail: document.querySelector(".rail")!, shelf: document.querySelector(".shelf")!, header: document.querySelector(".header")!, drawer };

  undrive = driveStates();

  return section;
};

describe("u133 click targets", () => {
  it("u133_every_click_target_shows_hover_pressed_and_a_24px_hit_area", async () => {
    const regions = await openEverything();
    const all = Object.entries(regions).flatMap(([region, root]) => targetsIn(root).map((el) => ({ region, el })));

    expect(new Set(all.map((one) => one.region))).toEqual(new Set(["rail", "shelf", "header", "drawer"]));
    expect(all.length).toBeGreaterThan(20);

    for (const { region, el } of all) {
      const name = `${region} ${el.tagName.toLowerCase()} ${el.getAttribute("aria-label") ?? el.textContent?.trim().slice(0, 20)}`;
      const rest = styleIn(el, "rest");
      const hover = styleIn(el, "hover");
      const pressed = styleIn(el, "pressed");
      const selected = el.getAttribute("data-selected") === "true";
      const paint = (style: CSSStyleDeclaration) => `${style.getPropertyValue("background-color")}|${style.getPropertyValue("color")}`;

      const cursor = "pointer";

      expect(hover.getPropertyValue("cursor"), name).toBe(cursor);
      expect(rest.getPropertyValue("cursor"), name).toBe(cursor);
      // The selected row's rest is already the wash a hover would draw.

      if (!selected) expect(paint(hover), `${name} hover`).not.toBe(paint(rest));
      expect(paint(pressed), `${name} pressed`).not.toBe(paint(hover));
      expect(pressed.getPropertyValue("transition-duration"), name).toBe("0s");
      expect(px(rest.getPropertyValue("min-height")), `${name} hit area`).toBeGreaterThanOrEqual(24);
    }
  });

  it("u133_the_prompt_field_hovers_presses_and_takes_the_same_ring", async () => {
    document.head.prepend(shell);
    render(() => <SpawnPromptField onSubmit={() => {}} onCancel={() => {}} ref={() => {}} />);
    undrive = driveStates();
    const field = screen.getByLabelText("prompt");

    expect(styleIn(field, "hover").getPropertyValue("background-color")).not.toBe(styleIn(field, "rest").getPropertyValue("background-color"));
    expect(styleIn(field, "pressed").getPropertyValue("background-color")).not.toBe(styleIn(field, "hover").getPropertyValue("background-color"));
    expect(outlineRulesIn(field, "focus")).toEqual(["[data-state-focus]"]);
  });

  it("u133_one_bare_focus_visible_rule_draws_a_2px_accent_ring_and_no_stylesheet_sets_another", async () => {
    const sheets = { "styles.css": appCss, "ink/styles.css": inkCss, "pads/styles.css": padsCss, "rail/styles.css": railCss, "terminal/styles.css": terminalCss, "todos/rowButton.styles.css": rowButtonCss, "rail/attentionChip.styles.css": chipCss, "tokens.css": tokenCss };
    const withOutline = Object.entries(sheets).flatMap(([file, css]) => [...css.replace(/\/\*[^]*?\*\//g, "").matchAll(/([^{}]+)\{([^{}]*(?<![\w-])outline(?:-[a-z]+)?\s*:[^{}]*)\}/g)].map((rule) => ({ file, selector: rule[1]!.trim(), body: rule[2]! })));
    const ring = withOutline.filter((rule) => /outline-color\s*:/.test(rule.body) && !/outline-width\s*:\s*0\s*;?\s*$/.test(rule.body.trim()));

    expect(ring).toHaveLength(1);
    expect(ring[0]!.file).toBe("styles.css");
    expect(ring[0]!.selector).toBe(":focus-visible");
    expect(ring[0]!.body).toMatch(/outline-width:\s*2px;\s*outline-style:\s*solid;\s*outline-color:\s*var\(--accent\)/);
    expect(withOutline.some((rule) => /outline(-style)?\s*:\s*none/.test(rule.body))).toBe(false);
    expect(withOutline.filter((rule) => rule !== ring[0])).toEqual([]);
  });

  it("u133_the_ring_is_3_to_1_against_the_ground_and_the_sunken_surface", () => {
    for (const ground of ["ground", "sunken"]) expect(contrastRatio(resolveToken("var(--accent)", tokens), resolveToken(`var(--${ground})`, tokens))).toBeGreaterThanOrEqual(3);
  });

  it("u133_a_row_the_prompt_the_drawer_buttons_and_a_todo_field_show_the_same_ring", async () => {
    const regions = await openEverything();
    const row = regions.rail.querySelector("[role=treeitem]")!;
    const close = within(regions.drawer).getByRole("button", { name: "close" });
    const fields = [document.body.appendChild(document.createElement("input")), document.body.appendChild(document.createElement("textarea"))];

    for (const el of [row, close, ...fields]) {
      expect(outlineRulesIn(el, "focus"), el.tagName).toEqual(["[data-state-focus]"]);
      expect(styleIn(el, "focus").getPropertyValue("outline-width")).toBe("2px");
      expect(styleIn(el, "focus").getPropertyValue("outline-offset")).toBe("1px");
    }
  });

  it("u133_stop_close_promote_collapse_and_export_md_are_buttons_with_a_24px_hit_area", async () => {
    const regions = await openEverything();
    fireEvent.click(regions.shelf.querySelector(".pad-name")!);
    const exportMd = await screen.findByRole("button", { name: "export .md" });
    const named = [...regions.rail.querySelectorAll("button"), ...regions.drawer.querySelectorAll("button")];
    const labels = named.map((el) => el.getAttribute("aria-label") ?? el.textContent?.trim());

    expect(labels).toEqual(expect.arrayContaining(["close", "collapse", "export .md"]));
    expect(exportMd.tagName).toBe("BUTTON");

    for (const el of named) expect(px(getComputedStyle(el).minHeight), `${el.getAttribute("aria-label") ?? el.textContent}`).toBeGreaterThanOrEqual(24);
  });

  it("u133_a_disabled_button_has_grey_text_cursor_default_and_aria_disabled", async () => {
    const controls = await mount("daemon-exits");
    controls.app.exitDaemon({ code: 1 });
    await waitFor(() => expect(document.querySelectorAll("button:disabled").length).toBeGreaterThan(0));
    undrive = driveStates();
    const disabled = [...document.querySelectorAll("button:disabled")];

    expect(disabled.length).toBeGreaterThan(0);

    for (const el of disabled) {
      expect(el.getAttribute("aria-disabled"), el.textContent ?? "").toBe("true");
      expect(styleIn(el, "rest").getPropertyValue("color")).toBe("var(--grey)");
      expect(styleIn(el, "rest").getPropertyValue("cursor")).toBe("default");
      expect(styleIn(el, "hover").getPropertyValue("color")).toBe("var(--grey)");
      expect(styleIn(el, "hover").getPropertyValue("cursor")).toBe("default");
    }
  });

  it("u133_a_rail_menu_button_reads_its_font_size_from_a_type_step", async () => {
    await mount("tree-40");
    fireEvent.contextMenu(screen.getAllByRole("treeitem")[0]!);
    const buttons = within(await screen.findByRole("menu")).getAllByRole("menuitem");

    expect(buttons.length).toBeGreaterThan(0);

    for (const button of buttons) expect(["13px", "var(--text-body)", "inherit"], button.textContent!).toContain(getComputedStyle(button).fontSize);
  });
});
