import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { colourOf, contrastRatio, loadTokens, withStylesheets } from "../testing/contrast";
import { mountTodos, todo, todoRowOf } from "./testHarness";

afterEach(cleanup);

describe("u87 contrast: Shelf", () => {
  it("u87_the_shelf_words_meet_4_5_to_1_on_sunken", async () => {
    const { tokens, sunken } = loadTokens();

    await withStylesheets(async () => {
      await mountTodos([todo(1)]);
      await screen.findByText(/todo 1/);
      fireEvent.mouseEnter(todoRowOf(1));

      expect(contrastRatio(colourOf(screen.getByText("complete"), tokens), sunken)).toBeGreaterThanOrEqual(4.5);
    });
  });
});
