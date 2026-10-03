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

    expect(rowOf("docs").querySelector(".name")!.compareDocumentPosition(live!) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
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

    expect([
      name.flexGrow,
      name.flexShrink,
      name.flexBasis,
      live.flexGrow,
      live.flexShrink,
      live.flexBasis,
      live.textAlign,
      live.whiteSpace,
    ]).toEqual(["0", "1", "auto", "1", "1", "0px", "right", "nowrap"]);
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
      "0px",
      "16px",
      "ellipsis",
      "hidden",
    ]);
  });

  it("u46_the_rows_height_is_28px_at_rest_hovered_and_selected", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;
    const mounted = await mountRail([agent("docs", "working", "writing docs/auth.md")]);
    const height = () => getComputedStyle(rowOf("docs")).minHeight;

    const rest = height();
    fireEvent.mouseEnter(rowOf("docs"));
    const hovered = height();
    mounted.rail.select("docs");
    const selected = height();

    sheet.remove();

    expect([rest, hovered, selected]).toEqual(["28px", "28px", "28px"]);
  });

  it("u46_every_item_after_the_first_keeps_an_8px_margin_except_the_live_line_which_has_none", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;
    await mountRail([agent("docs", "working", "writing docs/auth.md")]);
    fireEvent.mouseEnter(rowOf("docs"));

    const margins = (selector: string) => getComputedStyle(rowOf("docs").querySelector(selector)!).marginLeft;
    const result = [margins(".name"), margins(".live")];

    sheet.remove();

    expect(result).toEqual(["8px", "0px"]);
  });
});
