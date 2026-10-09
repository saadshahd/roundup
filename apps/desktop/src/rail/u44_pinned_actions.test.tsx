import { cleanup, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { agent } from "../testing/nodes";
import styles from "./styles.css?inline";
import { mountRail, rowOf } from "./railFixture";

afterEach(cleanup);

describe("u44 a row brought into view is never under the pinned actions", () => {
  it("u44_the_rail_reserves_the_actions_bars_own_height_below_every_row", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;
    await mountRail([agent("a", "idle", "i")]);

    const bar = getComputedStyle(screen.getByText("agent").closest(".rail-actions") ?? document.body);
    const row = getComputedStyle(rowOf("a"));

    const values = [
      bar.height,
      row.scrollMarginBottom,
      bar.getPropertyValue("--rail-actions-height"),
      row.getPropertyValue("--rail-actions-height"),
    ];

    sheet.remove();

    expect(values).toEqual(["var(--rail-actions-height)", "var(--rail-actions-height)", "40px", "40px"]);
  });
});
