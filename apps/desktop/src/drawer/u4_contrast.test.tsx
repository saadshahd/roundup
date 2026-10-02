import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { colourOf, contrastRatio, loadTokens, withStylesheets } from "../testing/contrast";
import { createDrawer } from "./drawer";
import { DrawerHost } from "./DrawerHost";

afterEach(cleanup);

describe("u4 ink contrast: Drawer", () => {
  it("u4_the_drawers_close_word_meets_4_5_to_1", async () => {
    const { tokens, ground } = loadTokens();

    await withStylesheets(() => {
      const drawer = createDrawer();
      render(() => <DrawerHost drawer={drawer} reducedMotion={() => true} />);
      drawer.open(() => <p>detail</p>);

      expect(contrastRatio(colourOf(screen.getByText("close"), tokens), ground)).toBeGreaterThanOrEqual(4.5);
    });
  });
});
