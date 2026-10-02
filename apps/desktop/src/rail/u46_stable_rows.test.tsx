import { cleanup, fireEvent } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { agent } from "../testing/nodes";
import styles from "./styles.css?inline";
import { mountRail, rowOf } from "./railFixture";

afterEach(cleanup);

describe("u46 hovering or selecting never moves a row", () => {
  it("u46_the_live_line_overlays_instead_of_pushing_the_row_below", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;
    await mountRail([agent("docs", "working", "writing docs/auth.md")]);

    const row = getComputedStyle(rowOf("docs"));

    fireEvent.mouseEnter(rowOf("docs"));
    const live = rowOf("docs").querySelector(".live");
    const liveStyle = live ? getComputedStyle(live) : null;

    sheet.remove();

    expect([row.position, liveStyle?.position]).toEqual(["relative", "absolute"]);
  });
});
