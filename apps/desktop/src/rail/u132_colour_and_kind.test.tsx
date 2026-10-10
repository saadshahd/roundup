import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { resolveToken, tokensFrom } from "../testing/contrast";
import { runChecks } from "../testing/checks";
import { agent } from "../testing/nodes";
import { seedApp } from "../testing/seeds";
import tokenCss from "../tokens.css?inline";
import appCss from "../styles.css?inline";
import { mountRail } from "./railFixture";

const tokens = tokensFrom(tokenCss);

const KINDS = ["error", "needs-you", "working", "blocked", "idle", "done"] as const;

// docs/design-system.md, "Colour".
const TABLE = { error: ["x", "red"], "needs-you": ["circle", "amber"], working: ["circle", "accent"], blocked: ["pause", "grey"], idle: ["dot", "grey"], done: ["check", "grey"] } as const;

const INK = new Set(["error", "needs-you"]);

const shell = document.createElement("style");

shell.textContent = `${tokenCss}\n${appCss}`;

afterEach(() => {
  shell.remove();
  cleanup();
});

const mountSeed = async (seed: "agents-10" | "tree-40") => {
  document.head.prepend(shell);
  const controls = seedApp(seed, Date.now());
  render(() => <App app={controls.app} reducedMotion={() => true} clock={Date.now} />);
  await screen.findAllByRole("treeitem");
};

const colourOf = (el: Element) => resolveToken(getComputedStyle(el).color, tokens);

const hueOf = (hex: string): number => {
  const channel = (at: number) => Number.parseInt(hex.slice(at, at + 2), 16) / 255;
  const [r, g, b] = [channel(1), channel(3), channel(5)];
  const [max, min] = [Math.max(r, g, b), Math.min(r, g, b)];
  const lightness = (max + min) / 2;

  return max === min ? 0 : (max - min) / (1 - Math.abs(2 * lightness - 1));
};

const glyphsOf = (root: ParentNode) => [...root.querySelectorAll<HTMLElement>('.glyph[role="img"]')];

/** The Kind a Glyph draws, from its shape and tone, so a decorative one (a row that names its Kind in text) counts too. */
const kindDrawn = (glyph: Element) => KINDS.find((kind) => glyph.querySelector("svg")?.classList.contains(`lucide-${TABLE[kind][0]}`) && glyph.getAttribute("data-tone") === TABLE[kind][1]) ?? null;

const drawnIn = (row: ParentNode) => [...row.querySelectorAll(".glyph")].flatMap((glyph) => kindDrawn(glyph) ?? []);

describe("u132 colour and Kind", () => {
  it("u132_each_glyph_is_the_one_the_design_system_assigns_its_kind_in_its_tone", async () => {
    await mountSeed("agents-10");
    // Ten Agents; the oldest `done` one folds into the `✓ <n> done` line (U8).
    expect(screen.getAllByRole("treeitem").length).toBeGreaterThanOrEqual(9);
    fireEvent.click(screen.getByRole("button", { name: /done/ }));
    expect(new Set(glyphsOf(document.body).map((glyph) => glyph.getAttribute("aria-label")))).toEqual(new Set(KINDS));

    for (const glyph of glyphsOf(document.body)) {
      // SAFETY: the `new Set(...)` assertion above has shown every label is one of KINDS.
      const [icon, tone] = TABLE[glyph.getAttribute("aria-label") as (typeof KINDS)[number]];

      expect(glyph.querySelector("svg")!.classList.contains(`lucide-${icon}`), glyph.getAttribute("aria-label")!).toBe(true);
      expect(colourOf(glyph), glyph.getAttribute("aria-label")!).toBe(resolveToken(`var(--${tone})`, tokens));
    }
  });

  // U172 replaces "only `needs-you` and `error` rows are bold": a title is bold by default, and Ink, told by tone and Glyph, stays on those two.
  it("u132_only_needs_you_and_error_rows_are_ink_and_every_title_is_bold", async () => {
    await mountSeed("agents-10");

    for (const row of screen.getAllByRole("treeitem")) {
      const kind = row.querySelector('.glyph[role="img"]')!.getAttribute("aria-label")!;
      const name = row.querySelector(".name")!;
      const weight = getComputedStyle(name).fontWeight;

      expect(Number(weight === "bold" ? 700 : weight), kind).toBeGreaterThanOrEqual(600);
      expect(name.classList.contains("ink"), kind).toBe(INK.has(kind));
    }
  });

  it("u132_every_saturated_colour_is_red_amber_or_accent", async () => {
    await mountSeed("agents-10");
    const allowed = ["red", "amber", "accent"].map((name) => resolveToken(`var(--${name})`, tokens).toLowerCase());
    const drawn = [...document.body.querySelectorAll("*")].flatMap((el) => ["color", "background-color", "border-top-color", "border-left-color", "outline-color"].map((property) => resolveToken(getComputedStyle(el).getPropertyValue(property), tokens).toLowerCase()));
    const saturated = drawn.filter((colour) => /^#[0-9a-f]{6}$/.test(colour) && hueOf(colour) > 0.15);

    expect(saturated.length).toBeGreaterThan(0);
    expect(saturated.filter((colour) => !allowed.includes(colour))).toEqual([]);
    expect(runChecks().D3.status).toBe("pass");
  });

  it("u132_a_rail_with_no_error_and_no_needs_you_draws_no_red_and_no_amber", async () => {
    document.head.prepend(shell);
    await mountRail([agent("a", "working", "w"), agent("b", "blocked", "w"), agent("c", "idle", "w"), agent("d", "done", "w")]);
    const [red, amber] = ["red", "amber"].map((name) => resolveToken(`var(--${name})`, tokens));
    const drawn = [...document.body.querySelectorAll("*")].flatMap((el) => ["color", "background-color", "border-top-color", "outline-color"].map((property) => resolveToken(getComputedStyle(el).getPropertyValue(property), tokens)));

    expect(glyphsOf(document.body)).toHaveLength(4);
    expect(drawn).not.toContain(red);
    expect(drawn).not.toContain(amber);
    expect(document.querySelectorAll(".ink")).toHaveLength(0);
    expect(runChecks().D3.status).toBe("pass");
  });

  it("u132_each_row_in_the_rail_the_shelf_and_a_drawer_shows_one_of_the_six_glyphs", async () => {
    await mountSeed("tree-40");
    const known = new Set<string | null>(KINDS);
    await screen.findAllByRole("button", { name: /#5/ });
    const shelfRows = [...document.querySelectorAll(".todo-row-button")];

    expect(shelfRows.length).toBeGreaterThan(0);

    // A Workstream has no Kind, so its row draws a chevron and no Kind Glyph.
    for (const row of [...screen.getAllByRole("treeitem").filter((item) => !item.hasAttribute("aria-expanded")), ...shelfRows])
      expect(drawnIn(row), row.outerHTML.slice(0, 300)).toHaveLength(1);

    fireEvent.click(shelfRows[0]!);
    const drawer = await screen.findByRole("complementary", { name: "drawer" });

    await waitFor(() => expect(drawnIn(drawer).length).toBeGreaterThan(0));
    expect(glyphsOf(drawer).every((glyph) => known.has(glyph.getAttribute("aria-label")))).toBe(true);
  });
});
