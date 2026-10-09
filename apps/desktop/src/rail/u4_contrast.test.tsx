import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { agent, room, NOW } from "../testing/nodes";
import { colourOf, contrastRatio, loadTokens, withStylesheets } from "../testing/contrast";
import { mountRail, rowOf } from "./railFixture";

afterEach(cleanup);

describe("u4 ink contrast: Rail", () => {
  it("u4_the_live_line_meets_4_5_to_1_on_the_ground_and_the_selected_band", async () => {
    const { tokens, ground, selected } = loadTokens();

    await withStylesheets(async () => {
      const { rail } = await mountRail([agent("blocker", "blocked", "waits on #4")]);
      const liveEl = () => rowOf("blocker").querySelector(".live");

      if (!liveEl()) throw new Error("no live line");

      const unselected = colourOf(liveEl()!, tokens);

      rail.select("blocker");
      const whenSelected = colourOf(liveEl()!, tokens);

      expect(contrastRatio(unselected, ground)).toBeGreaterThanOrEqual(4.5);
      expect(contrastRatio(whenSelected, selected)).toBeGreaterThanOrEqual(4.5);
    });
  });

  it.each([
    ["working", "working"],
    ["idle", "idle"],
  ] as const)("u4_the_%s_glyph_keeps_3_to_1_on_both_backgrounds", async (kind, mark) => {
    const { tokens, ground, selected } = loadTokens();

    await withStylesheets(async () => {
      const { rail } = await mountRail([agent("a", kind, "busy")]);
      const glyphEl = () => rowOf("a").querySelector(".glyph");

      expect(glyphEl()?.getAttribute("aria-label")).toBe(mark);
      expect(contrastRatio(colourOf(glyphEl()!, tokens), ground)).toBeGreaterThanOrEqual(3);

      rail.select("a");
      expect(contrastRatio(colourOf(glyphEl()!, tokens), selected)).toBeGreaterThanOrEqual(3);
    });
  });

  it.each(["needs-you", "error"] as const)(
    "u4_a_%s_name_in_ink_meets_4_5_to_1_on_both_backgrounds",
    async (kind) => {
      const { tokens, ground, selected } = loadTokens();

      await withStylesheets(async () => {
        const { rail } = await mountRail([agent("a", kind, "label")]);
        const nameEl = () => rowOf("a").querySelector(".name");

        expect(contrastRatio(colourOf(nameEl()!, tokens), ground)).toBeGreaterThanOrEqual(4.5);

        rail.select("a");
        expect(contrastRatio(colourOf(nameEl()!, tokens), selected)).toBeGreaterThanOrEqual(4.5);
      });
    },
  );

  it("u4_a_groups_promote_word_meets_4_5_to_1", async () => {
    const { tokens, ground } = loadTokens();

    await withStylesheets(async () => {
      await mountRail([room("g")]);
      fireEvent.mouseEnter(rowOf("g"));

      expect(contrastRatio(colourOf(screen.getByText("start Door"), tokens), ground)).toBeGreaterThanOrEqual(4.5);
    });
  });

  it("u4_the_done_fold_lines_grey_text_meets_4_5_to_1_unlike_an_unfolded_rows_lightest_glyph", async () => {
    const { tokens, ground } = loadTokens();

    await withStylesheets(async () => {
      await mountRail([
        agent("old", "done", "finished", { status: { kind: "done", label: "finished", since: NOW - 11 * 60_000 } }),
      ]);
      const foldLine = screen.getByText(/done$/).closest("button");

      if (!foldLine) throw new Error("no fold line button");

      expect(contrastRatio(colourOf(foldLine, tokens), ground)).toBeGreaterThanOrEqual(4.5);
    });
  });
});
