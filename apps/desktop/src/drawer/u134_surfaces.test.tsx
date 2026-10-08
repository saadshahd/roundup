import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { runChecks } from "../testing/checks";
import { resolveToken, tokensFrom } from "../testing/contrast";
import { seedApp } from "../testing/seeds";
import tokenCss from "../tokens.css?inline";
import appCss from "../styles.css?inline";
import inkCss from "../ink/styles.css?inline";
import padsCss from "../pads/styles.css?inline";
import railCss from "../rail/styles.css?inline";
import terminalCss from "../terminal/styles.css?inline";
import rowButtonCss from "../todos/rowButton.styles.css?inline";
import chipCss from "../rail/attentionChip.styles.css?inline";

const tokens = tokensFrom(tokenCss);

const sheet = (() => {
  const style = document.createElement("style");
  style.textContent = [tokenCss, appCss, inkCss, padsCss, railCss, terminalCss, rowButtonCss, chipCss].join("\n");

  return style;
})();

afterEach(() => {
  sheet.remove();
  cleanup();
});

const openDrawer = async () => {
  document.head.prepend(sheet);
  const controls = seedApp("tree-40", Date.now());
  render(() => <App app={controls.app} reducedMotion={() => true} clock={Date.now} />);
  fireEvent.click((await screen.findAllByRole("button", { name: /#5/ }))[0]!);

  return screen.getByRole("complementary", { name: "drawer" });
};

const read = (el: Element, property: string) => resolveToken(getComputedStyle(el).getPropertyValue(property), tokens);

describe("u134 surfaces and depth", () => {
  it("u134_the_rail_and_shelf_are_sunken_and_the_pane_and_drawer_are_ground", async () => {
    const drawer = await openDrawer();

    for (const region of [".rail", ".shelf"]) expect(read(document.querySelector(region)!, "background-color"), region).toBe(tokens.get("sunken"));
    expect(read(document.querySelector(".centre")!, "background-color")).toBe(tokens.get("ground"));
    expect(read(drawer, "background-color")).toBe(tokens.get("ground"));
  });

  it("u134_surfaces_inside_the_rail_and_shelf_do_not_paint_a_second_ground", async () => {
    await openDrawer();
    const pinned = [".rail-actions", ".rail-tree > .ink", ".shelf .complete"].flatMap((selector) => [...document.querySelectorAll(selector)]);

    for (const el of pinned) expect(read(el, "background"), el.className).toBe(tokens.get("sunken"));
  });

  it("u134_no_border_is_wider_than_1px_or_any_colour_but_hairline", async () => {
    await openDrawer();

    for (const el of document.body.querySelectorAll("*"))
      for (const side of ["top", "right", "bottom", "left"]) {
        if (getComputedStyle(el).getPropertyValue(`border-${side}-style`) in { none: 1, "": 1 }) continue;
        expect(read(el, `border-${side}-width`), el.className).toMatch(/^(0|1px)$/);
        expect(getComputedStyle(el).getPropertyValue(`border-${side}-color`), el.className).toBe("var(--hairline)");
      }

    expect(runChecks().D8.status).toBe("pass");
  });

  it("u134_only_the_drawer_has_a_box_shadow", async () => {
    const drawer = await openDrawer();
    const shadowed = [...document.body.querySelectorAll("*")].filter((el) => !["", "none"].includes(getComputedStyle(el).boxShadow));

    expect(shadowed).toEqual([drawer]);
    expect(getComputedStyle(drawer).boxShadow).toBe("var(--shadow-drawer)");

    fireEvent.click(screen.getByRole("button", { name: "close", hidden: false }));
    await waitFor(() => expect(drawer.inert).toBe(true));
    expect(["", "none"]).toContain(getComputedStyle(drawer).boxShadow);
  });

  it("u134_no_backdrop_filter_element_has_another_inside_it", async () => {
    await openDrawer();
    const backdrop = (el: Element) => !["", "none"].includes(getComputedStyle(el).getPropertyValue("backdrop-filter"));

    for (const el of document.body.querySelectorAll("*")) if (backdrop(el)) expect([...el.querySelectorAll("*")].filter(backdrop)).toEqual([]);
  });

  it("u134_the_drawer_floats_over_the_pane_so_the_terminal_does_not_resize", async () => {
    const drawer = await openDrawer();

    expect(getComputedStyle(drawer).position).toBe("absolute");
    expect(getComputedStyle(document.querySelector(".centre")!).position).not.toBe("absolute");
  });
});
