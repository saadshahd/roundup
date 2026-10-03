import { cleanup, screen } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import { afterEach, describe, expect, it } from "vitest";
import type { DaemonExit } from "../app/seam";
import rail from "./styles.css?inline";
import {
  applyFilter,
  colourOf,
  contrastRatio,
  greyedRailFilter,
  greyedRailRule,
  loadTokens,
  withStylesheets,
} from "../testing/contrast";
import { glyphOf, shownLiveLine, mountRail, rowNames, rowOf } from "./railFixture";
import { agent } from "../testing/nodes";

afterEach(cleanup);

describe("u25 the Daemon is gone", () => {
  it("u25_the_rail_is_dimmed_and_every_row_keeps_its_glyph_and_live_line", async () => {
    const [exit, setExit] = createSignal<DaemonExit | null>(null);

    await mountRail(
      [agent("a", "needs-you", "approve edit"), agent("b", "working", "reading", { order: 1 })],
      [],
      exit,
    );

    const tree = screen.getByRole("tree");
    const before = { glyph: glyphOf("a").getAttribute("aria-label"), line: shownLiveLine("a") };

    expect(tree.getAttribute("aria-disabled")).toBeNull();
    setExit({ code: 1 });

    expect(tree.getAttribute("aria-disabled")).toBe("true");
    expect(rowNames()).toEqual(["a", "b"]);
    expect({ glyph: glyphOf("a").getAttribute("aria-label"), line: shownLiveLine("a") }).toEqual(before);
  });

  it("u25_the_dim_is_a_grayscale_filter_and_never_an_opacity", () => {
    const rule = greyedRailRule(rail);
    const filter = greyedRailFilter(rail);

    expect(filter).toContain("grayscale(1)");
    expect(rule).not.toContain("opacity");
  });

  it.each([
    ["unselected, against the ground", false],
    ["selected, against the band", true],
  ] as const)("u25_the_live_line_still_clears_4_5_to_1_once_greyed_when_%s", async (_label, selectRow) => {
    const { tokens, ground, selected } = loadTokens();
    const [exit, setExit] = createSignal<DaemonExit | null>(null);

    await withStylesheets(async () => {
      const { rail: connected } = await mountRail([agent("blocker", "blocked", "waits on #4")], [], exit);
      const liveEl = () => rowOf("blocker").querySelector(".live");

      if (!liveEl()) throw new Error("no live line");

      if (selectRow) connected.select("blocker");

      const textBefore = colourOf(liveEl()!, tokens);

      setExit({ code: 1 });
      expect(screen.getByRole("tree").getAttribute("aria-disabled")).toBe("true");

      const filter = greyedRailFilter(rail);
      const text = applyFilter(textBefore, filter);
      const background = selectRow ? applyFilter(selected, filter) : ground;

      expect(contrastRatio(text, background)).toBeGreaterThanOrEqual(4.5);
    });
  });
});
