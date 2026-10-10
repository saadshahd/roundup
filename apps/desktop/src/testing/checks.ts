// U137: the one module that measures D1 to D10 (docs/design-system.md, "The checks"). `window.__checks()` on the harness page and the u130_ to u137_ tests call it, so a check has one implementation.
// It reads computed style, stylesheet text and the DOM and writes nothing: a hover, press or focus style is read from the stylesheet rules that would apply, never by driving the page into that state.
import { contrastRatio } from "./contrast";
import { inlineStyleCss, lookLiterals } from "./lookLiterals";

export const CHECK_IDS = ["D1", "D2", "D3", "D4", "D5", "D6", "D7", "D8", "D9", "D10"] as const;

export type CheckId = (typeof CHECK_IDS)[number];

/** One entry of a `checks.json` file, the shape `loop/rules.sh delta` reads (L42). */
export type Check = { status: "pass" | "fail"; value: unknown; selector: string };

export type Checks = Record<CheckId, Check>;

const NOT_MEASURABLE = "not measurable";

const TYPE_STEPS = new Set([11, 12, 13, 15]);

const SPACE_STEPS = new Set([0, 4, 8, 12, 16, 24, 32]);

// docs/motion.md, in seconds. `0` is an instant change.
const MOTION_SECONDS = [0, 0.12, 0.15, 0.18, 0.25, 0.4];

const CLICK_TARGET = "button, a[href], input, select, textarea, summary, [role=button], [role=treeitem], [role=tab]";

type Rgba = { r: number; g: number; b: number; a: number };

const parseColour = (value: string): Rgba | null => {
  const hex = /^#([0-9a-f]{6})$/i.exec(value.trim())?.[1];

  if (hex) return { r: parseInt(hex.slice(0, 2), 16), g: parseInt(hex.slice(2, 4), 16), b: parseInt(hex.slice(4, 6), 16), a: 1 };

  const parts = /^rgba?\(([^)]+)\)$/.exec(value.trim())?.[1]?.split(/[\s,/]+/).filter(Boolean).map(Number);

  if (!parts || parts.length < 3 || parts.some(Number.isNaN)) return null;

  return { r: parts[0]!, g: parts[1]!, b: parts[2]!, a: parts[3] ?? 1 };
};

const toHex = ({ r, g, b }: Rgba): string => `#${[r, g, b].map((channel) => Math.round(channel).toString(16).padStart(2, "0")).join("")}`;

const saturation = ({ r, g, b }: Rgba): number => {
  const [max, min] = [Math.max(r, g, b) / 255, Math.min(r, g, b) / 255];
  const lightness = (max + min) / 2;

  return max === min ? 0 : (max - min) / (1 - Math.abs(2 * lightness - 1));
};

const over = (top: Rgba, ground: Rgba): Rgba => ({
  r: top.r * top.a + ground.r * (1 - top.a),
  g: top.g * top.a + ground.g * (1 - top.a),
  b: top.b * top.a + ground.b * (1 - top.a),
  a: 1,
});

const px = (value: string): number | null => (/^-?[\d.]+px$/.test(value.trim()) ? Number.parseFloat(value) : value.trim() === "0" ? 0 : null);

const seconds = (value: string): number[] =>
  value.split(",").map((part) => part.trim()).filter(Boolean).map((part) => (part.endsWith("ms") ? Number.parseFloat(part) / 1000 : Number.parseFloat(part)));

const selectorOf = (el: Element): string => {
  const id = el.getAttribute("data-id");

  return `${el.tagName.toLowerCase()}${el.id ? `#${el.id}` : ""}${el.classList[0] ? `.${el.classList[0]}` : ""}${id ? `[data-id="${id}"]` : ""}`;
};

type Measure = { ok: boolean; value: unknown; selector: string };

type Page = {
  doc: Document;
  view: Window;
  elements: Element[];
  style: (el: Element) => CSSStyleDeclaration;
  token: (name: string) => string | undefined;
  resolve: (value: string) => string;
};

const styleRulesOf = (doc: Document): CSSStyleRule[] => {
  const walk = (rules: CSSRuleList): CSSStyleRule[] =>
    // SAFETY: a rule with `selectorText` is a CSSStyleRule, and one with `cssRules` is a grouping rule.
    [...rules].flatMap((rule) => ("selectorText" in rule ? [rule as CSSStyleRule] : "cssRules" in rule ? walk((rule as CSSGroupingRule).cssRules) : []));

  return [...doc.styleSheets].flatMap((sheet) => {
    try {
      return walk(sheet.cssRules);
    } catch {
      return [];
    }
  });
};

const rootTokens = (doc: Document): Map<string, string> => {
  const tokens = new Map<string, string>();

  for (const rule of styleRulesOf(doc))
    if (rule.selectorText.trim() === ":root") for (const [, name, value] of rule.cssText.matchAll(/--([\w-]+):\s*([^;}]+)/g)) tokens.set(name!, value!.trim());

  return tokens;
};

const pageOf = (doc: Document): Page => {
  const view = doc.defaultView!;
  const tokens = rootTokens(doc);
  const rootStyle = view.getComputedStyle(doc.documentElement);
  const token = (name: string) => rootStyle.getPropertyValue(`--${name}`).trim() || tokens.get(name);
  const resolve = (value: string): string => value.replace(/var\(--([\w-]+)\)/g, (whole, name: string) => token(name) ?? whole);
  const style = (el: Element) => view.getComputedStyle(el);
  const visible = (el: Element) => !el.closest("[hidden], [aria-hidden=true]") && style(el).display !== "none";

  return { doc, view, elements: [...doc.body.querySelectorAll("*")].filter((el) => !/^(SCRIPT|STYLE)$/.test(el.tagName) && visible(el)), style, token, resolve };
};

const inTerminal = (el: Element) => el.closest(".xterm, [data-terminal]") !== null;

const colourProperties = ["color", "background-color", "border-top-color", "border-right-color", "border-bottom-color", "border-left-color"] as const;

// D1: nothing but a Token read supplies a colour, font size, radius, shadow or duration.
const d1 = ({ doc, elements }: Page): Measure => {
  const found = styleRulesOf(doc)
    .filter((rule) => rule.selectorText.trim() !== ":root")
    .flatMap((rule) => lookLiterals(rule.cssText.slice(rule.cssText.indexOf("{"))).map((literal) => ({ selector: rule.selectorText, literal })));

  const inline = elements.flatMap((el) => lookLiterals(inlineStyleCss(`style={{${(el.getAttribute("style") ?? "").split(";").filter(Boolean).map((pair) => pair.replace(/^\s*([^:]+):\s*(.*)$/, '"$1": "$2"')).join(",")}}}`)).map((literal) => ({ selector: selectorOf(el), literal })));
  const all = [...found, ...inline];

  return { ok: all.length === 0, value: all.map((one) => one.literal), selector: all[0]?.selector ?? "" };
};

const offender = (page: Page, test: (el: Element, style: CSSStyleDeclaration) => object | null): Measure => {
  const hit = page.elements.map((el) => ({ el, found: test(el, page.style(el)) })).find((one) => one.found);

  return { ok: !hit, value: hit ? hit.found : [], selector: hit ? selectorOf(hit.el) : "" };
};

// D2: font sizes are type steps and margin, padding and gap are space steps.
const d2 = (page: Page): Measure =>
  offender(page, (el, style) => {
    if (inTerminal(el)) return null;

    const size = px(page.resolve(style.fontSize));

    if (size !== null && !TYPE_STEPS.has(size)) return { fontSize: style.fontSize };

    for (const property of ["margin-top", "margin-right", "margin-bottom", "margin-left", "padding-top", "padding-right", "padding-bottom", "padding-left", "row-gap", "column-gap"]) {
      const value = px(page.resolve(style.getPropertyValue(property)));

      if (value !== null && !SPACE_STEPS.has(value)) return { [property]: style.getPropertyValue(property) };
    }

    return null;
  });

const colourTokens = (page: Page): Rgba[] | null => {
  const read = ["red", "amber", "accent"].map((name) => parseColour(page.token(name) ?? ""));

  // SAFETY: `every(Boolean)` has just shown no entry is null.
  return read.every(Boolean) ? (read as Rgba[]) : null;
};

const sameColour = (a: Rgba, b: Rgba) => toHex(a) === toHex(b);

// D3: a saturated colour is red, amber or accent; with no error or needs-you row on screen, no red or amber.
const d3 = (page: Page): Measure => {
  const allowed = colourTokens(page);

  if (!allowed) return { ok: false, value: NOT_MEASURABLE, selector: "" };

  // SAFETY: `colourTokens` returns exactly red, amber and accent, in that order.
  const [red, amber] = allowed as [Rgba, Rgba, Rgba];
  const calm = page.elements.every((el) => !/^(error|needs-you)$/.test(el.getAttribute("aria-label") ?? "") && !/error|needs-you/.test(el.getAttribute("data-kind") ?? ""));

  return offender(page, (el, style) => {
    for (const property of colourProperties) {
      const colour = parseColour(page.resolve(style.getPropertyValue(property)));

      if (!colour || colour.a === 0) continue;

      if (saturation(colour) > 0.15 && !allowed.some((token) => sameColour(token, colour))) return { [property]: style.getPropertyValue(property) };

      if (calm && (sameColour(red, colour) || sameColour(amber, colour))) return { [property]: style.getPropertyValue(property), calm: true };
    }

    return null;
  });
};

// D4: every Rail, Shelf and Drawer row draws a Glyph, so no Kind is carried by colour alone.
const d4 = ({ doc }: Page): Measure => {
  const rows = [...doc.querySelectorAll("[role=treeitem], aside li, aside [role=listitem]")];
  const bare = rows.find((row) => !row.querySelector(".glyph svg, .glyph"));

  return { ok: !bare, value: { rows: rows.length }, selector: bare ? selectorOf(bare) : "" };
};

const groundOf = (page: Page, el: Element): Rgba => {
  let ground: Rgba = parseColour(page.token("ground") ?? "") ?? { r: 255, g: 255, b: 255, a: 1 };
  const chain: Element[] = [];

  for (let node: Element | null = el; node; node = node.parentElement) chain.unshift(node);

  for (const node of chain) {
    const background = parseColour(page.resolve(page.style(node).backgroundColor));

    if (background && background.a > 0) ground = over(background, ground);
  }

  return ground;
};

// D5: text 4.5:1, a Glyph tone and a focus ring 3:1 against their own ground; `done` is exempt (U4).
const d5 = (page: Page): Measure => {
  const ratio = (el: Element, property: string) => {
    const colour = parseColour(page.resolve(page.style(el).getPropertyValue(property)));

    return colour ? contrastRatio(toHex(over(colour, groundOf(page, el))), toHex(groundOf(page, el))) : null;
  };

  const ink = (el: Element) => [...el.childNodes].some((node) => node.nodeType === 3 && (node.textContent ?? "").trim() !== "");
  const focused = page.doc.activeElement && page.doc.activeElement !== page.doc.body && page.style(page.doc.activeElement).outlineStyle !== "none" ? page.doc.activeElement : null;

  return offender(page, (el) => {
    if (el.matches(".glyph") && el.closest('[aria-label="done"]') === null && !el.querySelector('[aria-label="done"]')) {
      const found = ratio(el, "color");

      if (found !== null && found < 3) return { glyph: found };
    } else if (ink(el)) {
      const found = ratio(el, "color");

      if (found !== null && found < 4.5) return { text: found };
    }

    if (el === focused) {
      const found = ratio(el, "outline-color");

      if (found !== null && found < 3) return { focusRing: found };
    }

    return null;
  });
};

// A rule that applies to `el` once its state pseudo-class is true: the selector without the pseudo-class matches it.
const stateRules = (page: Page, el: Element, pseudo: ":hover" | ":active" | ":focus-visible") =>
  styleRulesOf(page.doc).filter((rule) =>
    rule.selectorText.split(",").some((selector) => {
      if (!selector.includes(pseudo)) return false;

      const bare = selector.split(pseudo).join("").trim() || "*";

      try {
        return el.matches(bare);
      } catch {
        return false;
      }
    }),
  );

const declares = (rules: CSSStyleRule[], ...properties: string[]) => rules.some((rule) => properties.some((property) => rule.style.getPropertyValue(property) !== ""));

const clickTargets = (page: Page) => page.elements.filter((el) => el.matches(CLICK_TARGET) && !el.hasAttribute("disabled") && el.getAttribute("aria-disabled") !== "true");

// D6: each click target has a hover and a focus style and a hit area of at least 24 px.
const d6 = (page: Page): Measure => {
  const targets = clickTargets(page);
  let measured = 0;

  for (const el of targets) {
    const style = page.style(el);
    const height = Math.max(el.getBoundingClientRect().height, px(page.resolve(style.height)) ?? 0, px(page.resolve(style.minHeight)) ?? 0);

    if (height > 0) measured += 1;

    if (height > 0 && height < 24) return { ok: false, value: { height }, selector: selectorOf(el) };

    if (!declares(stateRules(page, el, ":hover"), "background", "background-color", "color")) return { ok: false, value: { missing: "hover" }, selector: selectorOf(el) };

    if (!declares(stateRules(page, el, ":focus-visible"), "outline", "outline-color", "box-shadow")) return { ok: false, value: { missing: "focus" }, selector: selectorOf(el) };

    // U155: a reorderable Todo row reads as grabbable.
    if (style.cursor !== (el.hasAttribute("data-reorderable") ? "grab" : "pointer")) return { ok: false, value: { cursor: style.cursor }, selector: selectorOf(el) };
  }

  return targets.length > 0 && measured === 0 ? { ok: false, value: NOT_MEASURABLE, selector: "" } : { ok: true, value: { targets: targets.length }, selector: "" };
};

// D7: each click target's pressed style differs from hover and has a transition of 0s.
const d7 = (page: Page): Measure => {
  for (const el of clickTargets(page)) {
    const pressed = stateRules(page, el, ":active");
    const hover = stateRules(page, el, ":hover");
    const paint = (rules: CSSStyleRule[]) => rules.map((rule) => `${rule.style.getPropertyValue("background")}|${rule.style.getPropertyValue("background-color")}|${rule.style.getPropertyValue("color")}`).join(";");
    const instant = pressed.some((rule) => /^0m?s$/.test(rule.style.getPropertyValue("transition-duration").trim() || rule.style.getPropertyValue("transition").trim().split(/\s+/).find((part) => /^[\d.]+m?s$/.test(part)) || ""));

    if (pressed.length === 0 || paint(pressed) === paint(hover) || !instant) return { ok: false, value: { pressed: pressed.length, instant }, selector: selectorOf(el) };
  }

  return { ok: true, value: { targets: clickTargets(page).length }, selector: "" };
};

// D8: no border over 1 px or off `--hairline`; only the Drawer has a shadow; no `backdrop-filter` element contains another.
const d8 = (page: Page): Measure => {
  const hairline = parseColour(page.token("hairline") ?? "");

  if (!hairline) return { ok: false, value: NOT_MEASURABLE, selector: "" };

  return offender(page, (el, style) => {
    for (const side of ["top", "right", "bottom", "left"]) {
      const width = px(style.getPropertyValue(`border-${side}-width`));
      const drawn = style.getPropertyValue(`border-${side}-style`) !== "none" && (width ?? 0) > 0;
      const colour = parseColour(page.resolve(style.getPropertyValue(`border-${side}-color`)));

      if (drawn && ((width ?? 0) > 1 || !colour || !sameColour(colour, hairline) || Math.abs(colour.a - hairline.a) > 0.01)) return { [`border-${side}`]: `${style.getPropertyValue(`border-${side}-width`)} ${style.getPropertyValue(`border-${side}-color`)}` };
    }

    const shadow = style.boxShadow;

    if (shadow && shadow !== "none" && !el.matches("aside, aside *") && el.tagName !== "ASIDE") return { boxShadow: shadow };

    const backdrop = (node: Element) => (page.style(node).getPropertyValue("backdrop-filter") || "none") !== "none";

    if (backdrop(el) && [...el.querySelectorAll("*")].some(backdrop)) return { backdropFilter: "nested" };

    return null;
  });
};

// D9: each duration is within 25% of docs/motion.md; under reduced motion all are 0.
const d9 = (page: Page): Measure => {
  const reduced = page.view.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;

  return offender(page, (el, style) => {
    const durations = [...seconds(style.transitionDuration), ...seconds(style.animationDuration)].map((duration) => (Number.isNaN(duration) ? 0 : duration));
    const bad = durations.find((duration) => (reduced ? duration !== 0 : !MOTION_SECONDS.some((step) => Math.abs(duration - step) <= step * 0.25)));

    return bad === undefined ? null : { duration: bad, reduced };
  });
};

const MEASURES: Record<Exclude<CheckId, "D10">, (page: Page) => Measure> = { D1: d1, D2: d2, D3: d3, D4: d4, D5: d5, D6: d6, D7: d7, D8: d8, D9: d9 };

/** D1 to D10 for what is on screen now, in the colour scheme and media the page is in. A check that cannot measure fails with `"not measurable"`. */
export function runChecks(doc: Document = document): Checks {
  const page = pageOf(doc);

  const measure = (id: Exclude<CheckId, "D10">): Check => {
    if (page.elements.length === 0) return { status: "fail", value: NOT_MEASURABLE, selector: "" };

    try {
      const { ok, value, selector } = MEASURES[id](page);

      return { status: ok ? "pass" : "fail", value, selector };
    } catch {
      return { status: "fail", value: NOT_MEASURABLE, selector: "" };
    }
  };

  // SAFETY: `MEASURES` has one entry for every id but D10, and `measure` returns a Check for each.
  const nine = Object.fromEntries((Object.keys(MEASURES) as Exclude<CheckId, "D10">[]).map((id) => [id, measure(id)])) as Record<Exclude<CheckId, "D10">, Check>;
  const failing = Object.entries(nine).filter(([, check]) => check.status === "fail").map(([id]) => id);
  const view = page.view;

  return {
    ...nine,
    D10: {
      status: failing.length === 0 ? "pass" : "fail",
      value: { failing, scheme: view.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light", contrast: view.matchMedia?.("(prefers-contrast: more)").matches ? "more" : "no-preference", viewport: `${view.innerWidth}x${view.innerHeight}` },
      selector: "",
    },
  };
}
