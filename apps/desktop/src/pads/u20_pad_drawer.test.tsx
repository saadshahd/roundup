import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { AGENT, openShelf, padOf, YOU } from "./padsFixture";

afterEach(cleanup);

const openPad = async (name: string) => {
  fireEvent.click(await screen.findByText(name));

  return screen.findByLabelText<HTMLTextAreaElement>("text");
};

const edit = (field: HTMLTextAreaElement, value: string) => {
  fireEvent.focus(field);
  fireEvent.input(field, { target: { value } });
  fireEvent.blur(field);
};

const methodsAfterOpen = (calls: string[]) => calls.filter((method) => method !== "rail.tree" && method !== "terminal.list" && method !== "pad.list");

describe("u20 open and edit", () => {
  it("u20_the_drawer_shows_the_owner_mark_the_name_and_who_owns_the_pad", async () => {
    await openShelf([padOf("auth-notes", AGENT, "x")]);
    await openPad("auth-notes");

    expect(screen.getByLabelText("drawer").textContent).toContain("◈ auth-notes owned by auth-refactor");
  });

  it("u20_the_drawer_offers_a_light_export_word", async () => {
    await openShelf([padOf("auth-notes", AGENT)]);
    await openPad("auth-notes");

    expect(screen.getByText("export .md").className).toBe("word");
  });

  it("u20_the_text_is_markdown_source_in_the_monospace_face", async () => {
    await openShelf([padOf("auth-notes", AGENT, "## refresh rotation\n- rotate on every use")]);

    const field = await openPad("auth-notes");

    expect([field.value, field.style.fontFamily]).toEqual(["## refresh rotation\n- rotate on every use", "var(--mono)"]);
  });

  it("u20_opening_calls_provenance_history_then_pad_read_once", async () => {
    const { calls } = await openShelf([padOf("auth-notes", AGENT)]);

    await openPad("auth-notes");

    expect(methodsAfterOpen(calls())).toEqual(["provenance.history", "pad.read"]);
  });

  it("u20_the_drawer_asks_for_the_history_of_the_pad_item", async () => {
    const { app } = await openShelf([padOf("auth-notes", AGENT)]);

    await openPad("auth-notes");

    expect(app.calls.find((call) => call.method === "provenance.history")?.params).toEqual({ item: "pad:auth-notes" });
  });

  it("u20_the_drawer_ends_with_the_last_touch_line", async () => {
    const { state } = await openShelf([padOf("auth-notes", AGENT)]);
    state.history = [{ actor: AGENT, verb: "wrote", item: "pad:auth-notes", at: new Date(2026, 0, 5, 14, 22).getTime() }];

    await openPad("auth-notes");

    expect((await screen.findByText(/^last/)).textContent).toBe("last  auth-refactor wrote 14:22");
  });

  it("u20_a_pad_the_user_owns_has_editable_text", async () => {
    await openShelf([padOf("release-checklist", YOU, "x")]);

    const field = await openPad("release-checklist");

    expect([field.readOnly, screen.queryByLabelText("append")]).toEqual([false, null]);
  });

  it("u20_leaving_the_field_calls_pad_write_once_when_the_text_changed", async () => {
    const { app } = await openShelf([padOf("release-checklist", YOU, "old")]);
    app.handlers["pad.write"] = ({ name, text }) => padOf(name, YOU, text);
    const field = await openPad("release-checklist");

    edit(field, "new");
    await screen.findByDisplayValue("new");

    expect(app.calls.filter((call) => call.method === "pad.write")).toEqual([
      { method: "pad.write", params: { name: "release-checklist", text: "new" } },
    ]);
  });

  it("u20_leaving_the_field_with_unchanged_text_calls_nothing", async () => {
    const { calls } = await openShelf([padOf("release-checklist", YOU, "same")]);
    const field = await openPad("release-checklist");

    edit(field, "same");

    expect(calls()).not.toContain("pad.write");
  });

  it("u20_a_pad_an_agent_owns_is_read_only_with_an_append_field", async () => {
    await openShelf([padOf("auth-notes", AGENT, "x")]);

    const field = await openPad("auth-notes");

    expect([field.readOnly, await screen.findByLabelText("append")].map(Boolean)).toEqual([true, true]);
  });

  it("u20_enter_in_the_append_field_calls_pad_append_and_clears_it", async () => {
    const { app } = await openShelf([padOf("auth-notes", AGENT, "a")]);
    app.handlers["pad.append"] = ({ name, text }) => padOf(name, AGENT, `a${text}`);
    await openPad("auth-notes");
    const append = await screen.findByLabelText<HTMLInputElement>("append");

    fireEvent.input(append, { target: { value: " more" } });
    fireEvent.keyDown(append, { key: "Enter" });
    await screen.findByDisplayValue("a more");

    expect(app.calls.filter((call) => call.method === "pad.append")).toEqual([
      { method: "pad.append", params: { name: "auth-notes", text: " more" } },
    ]);
    await waitFor(() => expect(append.value).toBe(""));
  });

  it("u20_pad_changed_for_the_open_pad_refreshes_its_text_from_pad_list", async () => {
    const { app, state } = await openShelf([padOf("auth-notes", AGENT, "v1")]);
    await openPad("auth-notes");
    state.pads = [padOf("auth-notes", AGENT, "v2")];

    app.emit({ actor: AGENT, name: "pad.changed", data: { name: "auth-notes" } });

    expect((await screen.findByDisplayValue("v2")).tagName).toBe("TEXTAREA");
  });

  it("u20_pad_changed_while_the_user_is_editing_leaves_the_text_alone", async () => {
    const { app, state } = await openShelf([padOf("release-checklist", YOU, "mine")]);
    const field = await openPad("release-checklist");
    fireEvent.focus(field);
    fireEvent.input(field, { target: { value: "mine, typing" } });
    state.pads = [padOf("release-checklist", YOU, "theirs")];

    app.emit({ actor: AGENT, name: "pad.changed", data: { name: "release-checklist" } });
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(field.value).toBe("mine, typing");
  });

  it("u20_pad_changed_for_another_pad_leaves_the_open_pad_alone", async () => {
    const { app, calls } = await openShelf([padOf("auth-notes", AGENT, "v1")]);
    await openPad("auth-notes");
    const before = calls().length;

    app.emit({ actor: AGENT, name: "pad.changed", data: { name: "other" } });
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(calls().slice(before)).toEqual(["pad.list"]);
  });
});
