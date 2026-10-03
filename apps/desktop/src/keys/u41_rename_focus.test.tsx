import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { agent } from "../testing/nodes";
import { railCallsTo, rowOf } from "../rail/railFixture";
import { mountKeys } from "./keysFixture";

afterEach(cleanup);

/** The name field has focus while it is open, as in the real window; jsdom does not move it by itself. */
const openedField = () => {
  const field = screen.getByLabelText<HTMLInputElement>("name");
  field.focus();

  return field;
};

const rowById = (id: string) => document.querySelector<HTMLElement>(`[data-id="${id}"]`);

const press = (key: string) => fireEvent.keyDown(document.activeElement ?? document, { key });

describe("u41 a rename gives focus back to the row", () => {
  it("u41_enter_in_the_name_field_returns_focus_to_the_renamed_row", async () => {
    const { app } = await mountKeys([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    app.handlers["rail.rename"] = () => agent("a", "idle", "x", { name: "renamed" });
    rowOf("a").focus();

    press("F2");
    const field = openedField();
    fireEvent.input(field, { target: { value: "renamed" } });
    fireEvent.keyDown(field, { key: "Enter" });

    await waitFor(() => expect(railCallsTo(app, "rail.rename")).toEqual([{ id: "a", name: "renamed" }]));
    await waitFor(() => expect(document.activeElement).toBe(rowById("a")));
  });

  it("u41_escape_in_the_name_field_returns_focus_to_the_row_and_calls_nothing", async () => {
    const { app } = await mountKeys([agent("a", "idle", "x")]);
    rowOf("a").focus();

    press("F2");
    fireEvent.keyDown(openedField(), { key: "Escape" });

    expect([railCallsTo(app, "rail.rename"), document.activeElement === rowOf("a")]).toEqual([[], true]);
  });

  it("u41_an_arrow_key_works_right_after_a_rename", async () => {
    const { app } = await mountKeys([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    app.handlers["rail.rename"] = () => agent("a", "idle", "x", { name: "renamed" });
    rowOf("a").focus();

    press("F2");
    fireEvent.keyDown(openedField(), { key: "Escape" });
    press("ArrowDown");

    expect(document.activeElement).toBe(rowOf("b"));
  });

  it("u41_a_rename_ended_by_a_click_elsewhere_leaves_focus_where_the_click_put_it", async () => {
    const { app } = await mountKeys([agent("a", "idle", "x"), agent("b", "idle", "x")]);
    rowOf("a").focus();

    press("F2");
    const field = openedField();
    fireEvent.input(field, { target: { value: "ignored" } });
    rowOf("b").focus();
    fireEvent.blur(field);

    expect([railCallsTo(app, "rail.rename"), document.activeElement === rowOf("b")]).toEqual([[], true]);
  });
});
