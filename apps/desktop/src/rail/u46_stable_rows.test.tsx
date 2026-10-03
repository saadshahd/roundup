import { cleanup, fireEvent } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { agent, metaAgent } from "../testing/nodes";
import styles from "./styles.css?inline";
import { mountRail, rowOf } from "./railFixture";

afterEach(cleanup);

describe("u46 hovering or selecting never moves a row", () => {
  it("u46_the_live_text_joins_the_rows_one_line_instead_of_a_second_one", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;
    await mountRail([agent("docs", "working", "writing docs/auth.md")]);

    const linesBefore = rowOf("docs").querySelectorAll("p").length;
    fireEvent.mouseEnter(rowOf("docs"));
    const live = rowOf("docs").querySelector(".live");
    const linesAfter = rowOf("docs").querySelectorAll("p").length;
    const liveStyle = live ? getComputedStyle(live) : null;

    sheet.remove();

    expect([linesBefore, linesAfter, live?.closest(".line") != null, liveStyle?.position]).toEqual([
      1,
      1,
      true,
      "static",
    ]);
  });

  it("u46_the_live_text_gets_only_what_the_name_leaves", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;
    await mountRail([agent("docs", "working", "writing docs/auth.md")]);
    fireEvent.mouseEnter(rowOf("docs"));

    const name = getComputedStyle(rowOf("docs").querySelector(".name")!);
    const live = getComputedStyle(rowOf("docs").querySelector(".live")!);

    sheet.remove();

    expect([name.flexGrow, live.flexGrow, live.textAlign, live.whiteSpace]).toEqual(["0", "1", "right", "nowrap"]);
  });

  it("u46_a_meta_agents_live_text_joins_the_first_line_too", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;
    await mountRail([metaAgent("lead", "blocked", "waits on #4")]);

    const linesAfter = rowOf("lead").querySelectorAll("p").length;
    const live = rowOf("lead").querySelector(".live");

    sheet.remove();

    expect([linesAfter, live?.closest(".line") != null]).toEqual([1, true]);
  });
});
