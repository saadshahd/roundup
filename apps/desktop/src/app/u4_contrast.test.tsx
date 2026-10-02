import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { createFakeApp } from "../testing/fakeApp";
import { colourOf, contrastRatio, loadTokens, withStylesheets } from "../testing/contrast";
import { App } from "../App";

afterEach(cleanup);

describe("u4 ink contrast: header", () => {
  it("u4_the_header_meets_4_5_to_1", async () => {
    const { tokens, ground } = loadTokens();

    await withStylesheets(async () => {
      const { container } = render(() => <App app={createFakeApp()} reducedMotion={() => false} clock={() => 0} />);

      await screen.findByText("roundup");
      const header = container.querySelector("header");

      if (!header) throw new Error("no header");

      expect(contrastRatio(colourOf(header, tokens), ground)).toBeGreaterThanOrEqual(4.5);
    });
  });
});
