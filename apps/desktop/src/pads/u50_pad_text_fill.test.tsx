import { cleanup, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { AGENT, openPad, openShelf, padOf } from "./padsFixture";
import { USER } from "../testing/nodes";

afterEach(cleanup);

describe("u50 the Pad's text fills its Drawer", () => {
  it("u50_the_text_field_grows_to_fill_the_drawers_remaining_height", async () => {
    await openShelf([padOf("auth-notes", AGENT)]);
    const field = await openPad("auth-notes");

    expect([field.style.flex, field.style.minHeight]).toEqual(["1 1 0%", "0px"]);
  });

  it("u50_the_text_field_has_no_native_resize_grip", async () => {
    await openShelf([padOf("auth-notes", AGENT)]);
    const field = await openPad("auth-notes");

    expect(field.style.resize).toBe("none");
  });

  it("u50_the_owned_text_field_sits_below_the_owner_line_and_above_the_last_touch_line", async () => {
    const { state } = await openShelf([padOf("release-checklist", USER, "x")]);
    state.history = [
      {
        actor: AGENT,
        verb: "wrote",
        item: "pad:release-checklist",
        at: new Date(2026, 0, 5, 14, 22).getTime(),
      },
    ];

    const field = await openPad("release-checklist");
    const owner = screen.getByText(/owned by/).closest("p")!;
    const lastTouch = await screen.findByText(/^last/);

    expect([
      !!(owner.compareDocumentPosition(field) & Node.DOCUMENT_POSITION_FOLLOWING),
      !!(field.compareDocumentPosition(lastTouch) & Node.DOCUMENT_POSITION_FOLLOWING),
    ]).toEqual([true, true]);
  });

  it("u50_the_agent_owned_text_field_sits_above_the_append_field_and_below_the_owner_line", async () => {
    const { state } = await openShelf([padOf("auth-notes", AGENT, "x")]);
    state.history = [
      {
        actor: AGENT,
        verb: "wrote",
        item: "pad:auth-notes",
        at: new Date(2026, 0, 5, 14, 22).getTime(),
      },
    ];

    const field = await openPad("auth-notes");
    const owner = screen.getByText(/owned by/).closest("p")!;
    const append = await screen.findByLabelText("append");
    const lastTouch = await screen.findByText(/^last/);

    expect([
      !!(owner.compareDocumentPosition(field) & Node.DOCUMENT_POSITION_FOLLOWING),
      !!(field.compareDocumentPosition(append) & Node.DOCUMENT_POSITION_FOLLOWING),
      !!(field.compareDocumentPosition(lastTouch) & Node.DOCUMENT_POSITION_FOLLOWING),
    ]).toEqual([true, true, true]);
  });

  it("u50_the_export_md_button_reads_in_grey", async () => {
    await openShelf([padOf("auth-notes", AGENT)]);
    await openPad("auth-notes");

    expect(screen.getByText("export .md").style.color).toBe("var(--grey)");
  });
});
