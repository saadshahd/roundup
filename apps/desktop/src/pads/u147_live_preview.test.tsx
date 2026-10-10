import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { AGENT, openPad, openShelf, padOf } from "./padsFixture";
import { USER } from "../testing/nodes";

afterEach(cleanup);

const writes = (calls: { method: string; params?: unknown }[]) => calls.filter((call) => call.method === "pad.write");

describe("u147 the Pad is one live-preview surface", () => {
  it("u147_a_user_owned_pad_is_one_editable_formatted_document", async () => {
    await openShelf([padOf("note", USER, "# Heading\n\n**bold** and *emphasis*")]);
    const field = await openPad("note");
    const document = screen.getByRole("textbox", { name: "Pad body" });

    expect(document.getAttribute("contenteditable")).toBe("true");
    expect(document.querySelector("h1")?.textContent).toBe("Heading");
    expect(document.querySelector("strong")?.textContent).toBe("bold");
    expect(document.textContent).not.toContain("**");
    expect(field.readOnly).toBe(false);
  });

  it("u147_no_control_switches_views", async () => {
    await openShelf([padOf("note", USER, "# Heading")]);
    await openPad("note");

    for (const name of ["edit", "Preview", "done", "source", "formatted"])
      expect(screen.queryByRole("button", { name })).toBeNull();
  });

  it("u147_an_agent_owned_pad_is_the_same_surface_read_only_with_append", async () => {
    const { app } = await openShelf([padOf("note", AGENT, "# Heading\n\ntext")]);
    const field = await openPad("note");
    const document = screen.getByRole("textbox", { name: "Pad body" });

    expect(document.getAttribute("contenteditable")).toBe("false");
    expect(document.querySelector("h1")?.textContent).toBe("Heading");
    expect(field.readOnly).toBe(true);

    const append = screen.getByLabelText("append");

    fireEvent.input(append, { target: { value: " more" } });
    fireEvent.keyDown(append, { key: "Enter" });

    await waitFor(() => expect(app.calls.filter((call) => call.method === "pad.append")).toHaveLength(1));
  });

  it("u147_blur_after_a_change_writes_once_and_an_unchanged_blur_writes_nothing", async () => {
    const { app } = await openShelf([padOf("note", USER, "old")]);
    const field = await openPad("note");

    fireEvent.focusOut(field);
    expect(writes(app.calls)).toHaveLength(0);

    fireEvent.input(field, { target: { value: "# New" } });
    fireEvent.focusOut(field);
    fireEvent.focusOut(field);

    await waitFor(() => expect(writes(app.calls)).toHaveLength(1));
    expect(writes(app.calls)[0]?.params).toEqual({ name: "note", text: "# New\n" });
  });

  it("u147_closing_the_drawer_after_a_change_writes_once", async () => {
    const { app, connected } = await openShelf([padOf("note", USER, "old")]);
    const field = await openPad("note");

    fireEvent.input(field, { target: { value: "changed" } });
    connected.drawer.close();

    await waitFor(() => expect(writes(app.calls)).toHaveLength(1));
    expect(writes(app.calls)[0]?.params).toEqual({ name: "note", text: "changed\n" });
  });

  it("u147_closing_the_drawer_after_a_blur_write_does_not_write_twice", async () => {
    const { app, connected } = await openShelf([padOf("note", USER, "old")]);
    const field = await openPad("note");

    fireEvent.input(field, { target: { value: "changed" } });
    fireEvent.focusOut(field);
    connected.drawer.close();

    await new Promise((done) => setTimeout(done, 0));
    expect(writes(app.calls)).toHaveLength(1);
  });

  it("u147_an_unchanged_document_keeps_its_source_bytes_and_writes_nothing", async () => {
    const source = "*emphasis*  \n\n# Heading\n\n";
    const { app, connected, state } = await openShelf([padOf("note", USER, source)]);
    const field = await openPad("note");

    fireEvent.focusOut(field);
    connected.drawer.close();

    await new Promise((done) => setTimeout(done, 0));
    expect(writes(app.calls)).toHaveLength(0);
    expect(state.pads[0]?.text).toBe(source);
  });

  it("u147_unsafe_markup_and_link_schemes_are_inert", async () => {
    await openShelf([
      padOf("note", AGENT, '<script>alert(1)</script><img src=x onerror=alert(2)> [bad](javascript:alert(3))'),
    ]);
    await openPad("note");
    const document = screen.getByRole("textbox", { name: "Pad body" });

    expect(document.querySelector("script, [onerror]")).toBeNull();
    expect(document.querySelector("a[href^='javascript:']")).toBeNull();
  });

  it("u147_the_editor_loads_only_when_a_drawer_opens", async () => {
    await openShelf([padOf("note", USER, "x")]);

    await screen.findByText("note");
    expect(document.querySelector(".milkdown")).toBeNull();

    await openPad("note");
    expect(document.querySelector(".milkdown")).not.toBeNull();
  });
});
