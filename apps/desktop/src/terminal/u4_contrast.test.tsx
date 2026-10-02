import { cleanup, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { info, node } from "../testing/nodes";
import { colourOf, contrastRatio, loadTokens, withStylesheets } from "../testing/contrast";
import { mountPane } from "./paneHarness";

afterEach(cleanup);

describe("u4 ink contrast: pane header", () => {
  it("u4_the_panes_stop_word_meets_4_5_to_1", async () => {
    const { tokens, ground } = loadTokens();

    await withStylesheets(async () => {
      const { connected, container } = await mountPane([node("a")], [info("t-a")]);
      connected.rail.select("a");

      expect(contrastRatio(colourOf(within(container).getByText("stop"), tokens), ground)).toBeGreaterThanOrEqual(
        4.5,
      );
    });
  });
});
