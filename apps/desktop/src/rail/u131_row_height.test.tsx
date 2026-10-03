import { cleanup, fireEvent } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { agent, group, metaAgent, terminal } from "../testing/nodes";
import styles from "./styles.css?inline";
import { mountRail, rowOf } from "./railFixture";

afterEach(cleanup);

const minHeightOf = (name: string) => getComputedStyle(rowOf(name)).minHeight;

describe("u131 a Rail row is 28 px high", () => {
  it("u131_every_kind_of_row_has_a_28px_minimum_height_at_rest", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;
    await mountRail([group("g"), agent("a", "working", "w"), metaAgent("m", "working", "w"), terminal("t")]);

    const heights = ["g", "a", "m", "t"].map(minHeightOf);

    sheet.remove();

    expect(heights).toEqual(["28px", "28px", "28px", "28px"]);
  });

  it("u131_hovering_or_selecting_a_row_keeps_it_28px", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;
    const { rail } = await mountRail([agent("a", "working", "w")]);

    fireEvent.mouseEnter(rowOf("a"));
    const hovered = minHeightOf("a");
    rail.select("a");
    const selected = minHeightOf("a");

    sheet.remove();

    expect([hovered, selected]).toEqual(["28px", "28px"]);
  });
});
