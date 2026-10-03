import { cleanup, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import paneStyles from "./styles.css?inline";
import { mountPane } from "./paneHarness";
import { info, node } from "../testing/nodes";

afterEach(cleanup);

const placingSelectors = (sheet: string): string[] =>
  [...sheet.matchAll(/([^{}]+)\{([^{}]*)\}/g)]
    .filter(([, , body]) => /(^|;|\s)position\s*:\s*absolute\b/.test(body!))
    .flatMap(([, selectors]) => selectors!.split(",").map((selector) => selector.trim()))
    .filter((selector) => selector.includes(".pane-latest"));

const classesIn = (selector: string): number => selector.match(/\.[\w-]+/g)?.length ?? 0;

describe("u34 the latest control is placed by a rule that no `.word` reset can beat", () => {
  // jsdom ignores `all: unset`, and which of two one-class rules wins depends on the order the bundler loads the
  // sheets, so the rule is required to out-weigh `.word` (one class) whatever that order is.
  it("u34_the_rule_that_positions_the_latest_control_names_at_least_two_classes", () => {
    const selectors = placingSelectors(paneStyles);

    expect(selectors.length).toBeGreaterThan(0);
    expect(selectors.filter((selector) => classesIn(selector) < 2)).toEqual([]);
  });

  it("u34_the_latest_control_carries_the_classes_that_rule_names", async () => {
    const { connected, emulators } = await mountPane([node("a")], [info("t-a")]);

    connected.rail.select("a");
    emulators.get("t-a")?.scroll(false);

    expect(screen.getByText("↓ latest").className).toBe("word pane-latest");
  });
});
