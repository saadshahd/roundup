import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { agent } from "../testing/nodes";
import { rowOf } from "../rail/railFixture";
import { mountKeys } from "./keysFixture";

afterEach(cleanup);

const press = (key: string) => fireEvent.keyDown(document.activeElement ?? document, { key });

const renamedRow = () => screen.getByLabelText("name").closest("[role=treeitem]")?.getAttribute("data-id");

describe("u140 F2 renames the focused row", () => {
  it("u140_f2_edits_the_focused_rows_name_even_when_another_row_is_selected", async () => {
    const { rail } = await mountKeys([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rail.select("b");
    rowOf("a").focus();

    press("F2");

    expect(renamedRow()).toBe("a");
  });

  it("u140_f2_edits_the_focused_row_when_nothing_is_selected", async () => {
    await mountKeys([agent("a", "idle", "x")]);
    rowOf("a").focus();

    press("F2");

    expect(renamedRow()).toBe("a");
  });

  it("u140_f2_after_arrow_down_renames_the_row_focus_moved_to", async () => {
    const { rail } = await mountKeys([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rail.select("a");
    rowOf("a").focus();

    press("ArrowDown");
    press("F2");

    expect(renamedRow()).toBe("b");
  });
});
