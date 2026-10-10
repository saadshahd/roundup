import { cleanup, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { colourOf, contrastRatio, loadTokens, withStylesheets } from "../testing/contrast";
import { AGENT, openPad, openShelf, padOf } from "./padsFixture";

afterEach(cleanup);

describe("u87 contrast: Pad Drawer", () => {
  it("u87_the_shelf_and_drawer_words_meet_4_5_to_1", async () => {
    const { tokens, ground } = loadTokens();

    await withStylesheets(async () => {
      await openShelf([padOf("auth-notes", AGENT)]);
      await openPad("auth-notes");

      for (const word of [screen.getByText("export .md"), screen.getByRole("button", { name: "close" })])
        expect(contrastRatio(colourOf(word, tokens), ground), word.textContent ?? "").toBeGreaterThanOrEqual(4.5);
    });
  });
});
