import { cleanup, fireEvent } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { agent, metaAgent, terminal } from "../testing/nodes";
import styles from "./styles.css?inline";
import { exitedTerminal, mountRail, rowOf } from "./railFixture";

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

  it("u46_an_exited_terminals_live_text_joins_the_first_line_and_keeps_its_title", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;
    await mountRail([terminal("dev")], [exitedTerminal("dev", 137)]);
    fireEvent.mouseEnter(rowOf("dev"));

    const live = rowOf("dev").querySelector(".live");
    const paragraphs = rowOf("dev").querySelectorAll("p").length;

    sheet.remove();

    expect([paragraphs, live?.closest(".line") != null, live?.getAttribute("title") === live?.textContent]).toEqual([1, true, true]);
  });

  it("u46_the_live_line_takes_no_width_from_the_name_and_is_cut_with_an_ellipsis", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;
    await mountRail([agent("docs", "working", "writing docs/auth.md")]);
    fireEvent.mouseEnter(rowOf("docs"));

    const live = getComputedStyle(rowOf("docs").querySelector(".live")!);

    sheet.remove();

    expect([live.minWidth, live.paddingLeft, live.marginLeft, live.textIndent, live.textOverflow, live.overflow]).toEqual([
      "0px",
      "0",
      "-8px",
      "16px",
      "ellipsis",
      "hidden",
    ]);
  });
});
