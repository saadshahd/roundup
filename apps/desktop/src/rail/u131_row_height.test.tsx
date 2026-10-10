import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { agent, workstream, door, MINUTE, NOW, terminal } from "../testing/nodes";
import { withStylesheets } from "../testing/contrast";
import styles from "./styles.css?inline";
import { mountRail, rowOf } from "./railFixture";

afterEach(cleanup);

const minHeightOf = (name: string) => getComputedStyle(rowOf(name)).minHeight;

describe("u131 a Rail row is 28 px high", () => {
  it("u131_the_rail_uses_the_ui_face_for_names_and_actions", async () => {
    await withStylesheets(async () => {
      await mountRail([agent("a", "working", "w")]);

      expect(getComputedStyle(rowOf("a").closest(".rail-tree")!).fontFamily).toBe("var(--font-ui)");
    });
  });

  it("u131_every_kind_and_state_of_row_has_a_28px_minimum_height_at_rest", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;
    const kinds = ["working", "idle", "done", "blocked", "needs-you", "error"] as const;
    await mountRail([
      workstream("g"),
      door("m", "working", "w"),
      terminal("t"),
      ...kinds.map((kind) => agent(kind, kind, "w")),
    ]);

    const heights = ["g", "m", "t", ...kinds].map(minHeightOf);

    sheet.remove();

    expect(heights).toEqual(Array(9).fill("28px"));
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

  it("u131_the_done_fold_line_is_28px_high_like_the_rows_around_it", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;
    await mountRail([
      agent("old", "done", "finished", { status: { kind: "done", label: "finished", since: NOW - 20 * MINUTE } }),
      agent("busy", "working", "w"),
    ]);

    const fold = getComputedStyle(screen.getByText("1 done").parentElement!);

    sheet.remove();

    expect(fold.minHeight).toBe("28px");
  });
});
