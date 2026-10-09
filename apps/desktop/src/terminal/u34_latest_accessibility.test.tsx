import { cleanup, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import paneStyles from "./styles.css?inline";
import { mountPane } from "./paneHarness";
import { colourOf, contrastRatio, loadTokens, resolveToken, withStylesheets } from "../testing/contrast";
import { info, node } from "../testing/nodes";

afterEach(cleanup);

describe("u34 latest control reads and answers over the emulator", () => {
  it("u34_the_latest_control_meets_text_contrast_and_the_minimum_hit_height", async () => {
    const { tokens } = loadTokens();
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = paneStyles;
    const rule = /\.pane \.pane-latest\s*\{([^}]*)\}/.exec(paneStyles)?.[1];

    expect(rule).toBeDefined();

    try {
      await withStylesheets(async () => {
        const { connected, emulators } = await mountPane([node("a")], [info("t-a")]);
        connected.rail.select("a");
        emulators.get("t-a")?.scroll(false);

        const control = screen.getByText("↓ latest");
        const ground = resolveToken(/background:\s*([^;]+);/.exec(rule!)?.[1] ?? "", tokens);
        const height = resolveToken(/min-height:\s*([^;]+);/.exec(rule!)?.[1] ?? "", tokens);

        expect(control.matches(".pane .pane-latest")).toBe(true);
        expect(contrastRatio(colourOf(control, tokens), ground)).toBeGreaterThanOrEqual(4.5);
        expect(parseFloat(height)).toBeGreaterThanOrEqual(24);
      });
    } finally {
      sheet.remove();
    }
  });
});
