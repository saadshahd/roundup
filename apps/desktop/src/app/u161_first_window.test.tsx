import { cleanup, fireEvent, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { seedApp } from "../testing/seeds";
import tokenCss from "../tokens.css?inline";
import appCss from "../styles.css?inline";
import inkCss from "../ink/styles.css?inline";
import padsCss from "../pads/styles.css?inline";
import railCss from "../rail/styles.css?inline";
import terminalCss from "../terminal/styles.css?inline";
import rowButtonCss from "../todos/rowButton.styles.css?inline";
import chipCss from "../rail/attentionChip.styles.css?inline";

let sheet: HTMLStyleElement | undefined;

afterEach(() => {
  sheet?.remove();
  cleanup();
});

const mount = async () => {
  sheet = document.head.appendChild(document.createElement("style"));
  sheet.textContent = [tokenCss, appCss, inkCss, padsCss, railCss, terminalCss, rowButtonCss, chipCss].join("\n");

  window.matchMedia ??= (query) => ({ matches: false, media: query, onchange: null, addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {}, dispatchEvent: () => false });

  const controls = seedApp("door-stopped", Date.now());

  render(() => <App app={controls.app} reducedMotion={() => false} clock={Date.now} />);
  fireEvent.click(await screen.findByText("first room"));
  await screen.findByText("Door stopped, exited 0");
};

const style = (selector: string) => getComputedStyle(document.querySelector(selector)!);

describe("u161 the first window reads as designed", () => {
  it("u161_a_hairline_on_the_rails_trailing_edge_and_the_shelfs_top_edge_and_no_other_region_edge", async () => {
    await mount();

    expect(style(".rail").borderRightWidth).toBe("1px");
    expect(style(".rail").borderRightColor).toBe("var(--hairline)");
    expect(style(".shelf").borderTopWidth).toBe("1px");
    expect(style(".shelf").borderTopColor).toBe("var(--hairline)");

    for (const [selector, edges] of [
      [".rail", ["Top", "Bottom", "Left"]],
      [".shelf", ["Right", "Bottom", "Left"]],
      [".centre", ["Top", "Right", "Bottom", "Left"]],
    ] as const)
      for (const edge of edges) expect(style(selector).getPropertyValue(`border-${edge.toLowerCase()}-width`), `${selector} ${edge}`).not.toBe("1px");
  });

  it("u161_every_region_pads_its_leading_and_trailing_edge_by_space_4", async () => {
    await mount();

    for (const selector of [".rail", ".centre", ".shelf"]) {
      expect(style(selector).paddingLeft, selector).toBe("var(--space-4)");
      expect(style(selector).paddingRight, selector).toBe("var(--space-4)");
    }
  });

  it("u161_the_add_row_words_are_space_2_apart_and_the_centre_group_space_3_apart", async () => {
    await mount();

    expect(style(".rail-actions").gap).toBe("var(--space-2)");
    expect(style(".pane-door").gap).toBe("var(--space-3)");
    expect(style(".pane-action").minHeight).toBe("28px");
  });

  it("u161_every_mark_is_16px_on_one_stroke_width_and_one_corner_style", async () => {
    await mount();

    const marks = [...document.querySelectorAll("svg.icon")];

    expect(marks.length).toBeGreaterThan(0);

    for (const mark of marks) {
      expect(mark.getAttribute("width")).toBe("16");
      expect(mark.getAttribute("height")).toBe("16");
      expect(mark.getAttribute("stroke-width")).toBe("1.75");
      expect(mark.getAttribute("stroke-linejoin")).toBe("round");
    }
  });
});
