import { cleanup, render } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { Button } from "../ink/Button";
import { driveStates, outlineRulesIn, styleIn } from "../testing/pointerStates";
import { contrastRatio } from "../testing/contrast";
import tokenCss from "../tokens.css?inline";
import appCss from "../styles.css?inline";
import buttonCss from "../buttons.css?inline";

const KINDS = ["primary", "quiet", "add", "row-action", "icon"] as const;

let undrive = () => {};

let sheet: HTMLStyleElement | undefined;

afterEach(() => {
  undrive();
  sheet?.remove();
  cleanup();
});

const mount = () => {
  sheet = document.head.appendChild(document.createElement("style"));
  sheet.textContent = [tokenCss, appCss, buttonCss].join("\n");

  render(() => (
    <div>
      {KINDS.map((kind) => (
        <>
          <Button kind={kind} aria-label={`${kind} on`}>{kind}</Button>
          <Button kind={kind} unavailable aria-label={`${kind} off`} title="why">{kind}</Button>
        </>
      ))}
    </div>
  ));
  undrive = driveStates();
};

const button = (name: string) => document.querySelector<HTMLElement>(`[aria-label="${name}"]`)!;

const paint = (style: CSSStyleDeclaration) => `${style.getPropertyValue("background-color")}|${style.getPropertyValue("color")}`;

const hexIn = (css: string, name: string) => new RegExp(`${name}:\\s*(#[0-9a-fA-F]{6})`).exec(css)?.[1] ?? "";

describe("u171 every kind shows every state", () => {
  it.each(KINDS)("u171_%s_hover_pressed_focus_and_rest_differ_and_the_press_is_instant", (kind) => {
    mount();
    const el = button(`${kind} on`);
    const [rest, hover, pressed] = [styleIn(el, "rest"), styleIn(el, "hover"), styleIn(el, "pressed")];

    expect(paint(hover)).not.toBe(paint(rest));
    expect(paint(pressed)).not.toBe(paint(hover));
    expect(pressed.getPropertyValue("transition-duration")).toBe("0s");
    expect([hover.getPropertyValue("cursor"), rest.getPropertyValue("cursor")]).toEqual(["pointer", "pointer"]);
    expect(outlineRulesIn(el, "focus").length).toBeGreaterThan(0);
    expect(Number.parseFloat(rest.getPropertyValue("min-height").replace("var(--space-5)", "24"))).toBeGreaterThanOrEqual(24);
  });

  it.each(KINDS)("u171_%s_disabled_keeps_aria_disabled_not_the_attribute_and_never_changes_on_hover_or_press", (kind) => {
    mount();
    const el = button(`${kind} off`);
    const rest = styleIn(el, "rest");

    expect([el.getAttribute("aria-disabled"), el.hasAttribute("disabled"), el.title]).toEqual(["true", false, "why"]);
    expect(rest.getPropertyValue("color")).toBe("var(--disabled)");
    expect(rest.getPropertyValue("cursor")).toBe("default");
    expect(paint(styleIn(el, "hover"))).toBe(paint(rest));
    expect(paint(styleIn(el, "pressed"))).toBe(paint(rest));
  });

  it("u171_no_two_kinds_have_the_same_rest_style", () => {
    mount();

    const looks = KINDS.map((kind) => {
      const style = getComputedStyle(button(`${kind} on`));

      return [style.color, style.fontWeight, style.fontSize, style.paddingLeft, style.minWidth].join("|");
    });

    expect(new Set(looks).size).toBe(KINDS.length);
  });

  it("u171_an_unavailable_button_does_not_act_but_stays_focusable", () => {
    let clicks = 0;

    render(() => <Button kind="quiet" unavailable aria-label="off" onClick={() => (clicks += 1)}>off</Button>);
    const el = button("off");
    el.focus();
    el.click();

    expect([clicks, document.activeElement === el]).toEqual([0, true]);
  });

  it("u171_the_disabled_tone_clears_4_5_to_1_on_the_ground_and_the_sunken_in_both_schemes", () => {
    const dark = /@media \(prefers-color-scheme: dark\)\s*\{([\s\S]*?)\n\}/.exec(tokenCss)?.[1] ?? "";
    const light = tokenCss.slice(0, tokenCss.indexOf("@media"));

    for (const scheme of [light, dark])
      for (const ground of ["--ground", "--sunken"]) expect(contrastRatio(hexIn(scheme, "--disabled"), hexIn(scheme, ground))).toBeGreaterThanOrEqual(4.5);
  });
});
