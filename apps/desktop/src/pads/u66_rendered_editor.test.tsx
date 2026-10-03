import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { EditorView } from "@codemirror/view";
import { AGENT, openPad, openShelf, padOf } from "./padsFixture";
import { USER } from "../testing/nodes";

afterEach(cleanup);

const open = async (name: string) => {
  fireEvent.click(await screen.findByText(name));

  return screen.findByLabelText("Pad body");
};

describe("u66 rendered Pads and a source Editor", () => {
  it("u66_the_editor_accepts_a_source_edit", async () => {
    await openShelf([padOf("note", USER, "old")]);
    const field = await openPad("note");

    expect(Object.getOwnPropertyDescriptor(field, "value")?.set).toBeTypeOf("function");
    fireEvent.input(field, { target: { value: "new" } });

    expect(field.value).toBe("new");
  });

  it("u66_opening_renders_markdown_structure_in_the_pad_body", async () => {
    await openShelf([
      padOf("note", AGENT, "# Heading\n\n**bold** and *emphasis*\n\n- item\n\n> quote\n\n```js\nconst n = 1\n```"),
    ]);
    const body = await open("note");

    expect(body.querySelector("h1")?.textContent).toBe("Heading");
    expect(body.querySelector("strong")?.textContent).toBe("bold");
    expect(body.querySelector("em")?.textContent).toBe("emphasis");
    expect(body.querySelector("li")?.textContent).toBe("item");
    expect(body.querySelector("blockquote")?.textContent?.trim()).toBe("quote");
    expect(body.querySelector("pre code")?.textContent).toBe("const n = 1\n");
  });

  it("u66_unsafe_markup_and_link_schemes_are_inert", async () => {
    await openShelf([
      padOf("note", AGENT, '<script>alert(1)</script><img src=x onerror=alert(2)> [bad](javascript:alert(3))'),
    ]);
    const body = await open("note");

    expect(body.querySelector("script")).toBeNull();
    expect(body.querySelector("[onerror]")).toBeNull();
    expect(body.querySelector("a[href^='javascript:']")).toBeNull();
    expect(body.textContent).toContain("bad");
  });

  it("u66_an_agent_owned_pad_has_no_edit_action", async () => {
    await openShelf([padOf("note", AGENT, "# Heading")]);
    await open("note");

    expect(screen.queryByRole("button", { name: "edit" })).toBeNull();
    expect(screen.getByLabelText("append")).toBeTruthy();
  });

  it("u66_a_user_owned_pad_opens_a_syntax_highlighted_editor", async () => {
    await openShelf([padOf("note", USER, "# Heading")]);
    await open("note");
    fireEvent.click(screen.getByRole("button", { name: "edit" }));
    fireEvent.click(await screen.findByRole("button", { name: "source" }));

    const editor = await screen.findByRole("textbox", { name: "Editor" });
    expect(editor.textContent).toContain("# Heading");
    expect(editor.querySelector(".cm-line span[class]")).not.toBeNull();
  });

  it("u66_preview_shows_the_draft_without_writing", async () => {
    const { app } = await openShelf([padOf("note", USER, "# Heading")]);
    await open("note");
    fireEvent.click(screen.getByRole("button", { name: "edit" }));
    fireEvent.click(screen.getByRole("button", { name: "Preview" }));

    expect((await screen.findByLabelText("Preview")).querySelector("h1")?.textContent).toBe("Heading");
    expect(app.calls.filter((call) => call.method === "pad.write")).toHaveLength(0);
  });

  it("u66_preview_renders_the_current_draft_without_writing", async () => {
    const { app } = await openShelf([padOf("note", USER, "# Old")]);
    const field = await openPad("note");

    fireEvent.input(field, { target: { value: "# New" } });
    fireEvent.click(screen.getByRole("button", { name: "Preview" }));

    expect((await screen.findByLabelText("Preview")).querySelector("h1")?.textContent).toBe("New");
    expect(app.calls.filter((call) => call.method === "pad.write")).toHaveLength(0);
  });

  it("u66_returning_from_preview_restores_the_exact_draft", async () => {
    await openShelf([padOf("note", USER, "old")]);
    const field = await openPad("note");
    const source = "# New\n\n- one\n- two\n";

    fireEvent.input(field, { target: { value: source } });
    fireEvent.click(screen.getByRole("button", { name: "Preview" }));
    await screen.findByLabelText("Preview");
    fireEvent.click(screen.getByRole("button", { name: "edit" }));
    fireEvent.click(await screen.findByRole("button", { name: "source" }));

    const editor = await screen.findByRole("textbox", { name: "Editor" });
    expect(EditorView.findFromDOM(editor)?.state.doc.toString()).toBe(source);
  });

  it("u66_done_writes_the_exact_changed_source_once", async () => {
    const { app } = await openShelf([padOf("note", USER, "old")]);
    const field = await openPad("note");
    const source = "# New\n\n- one\n- two\n";

    fireEvent.input(field, { target: { value: source } });
    fireEvent.click(screen.getByRole("button", { name: "done" }));

    await waitFor(() => expect(app.calls.filter((call) => call.method === "pad.write")).toHaveLength(1));
    expect(app.calls.find((call) => call.method === "pad.write")?.params).toEqual({ name: "note", text: source });
  });

  it("u66_done_without_a_change_does_not_write", async () => {
    const { app } = await openShelf([padOf("note", USER, "# Heading")]);
    await open("note");
    fireEvent.click(screen.getByRole("button", { name: "edit" }));
    fireEvent.click(screen.getByRole("button", { name: "done" }));

    await waitFor(() => expect(screen.getByLabelText("Pad body").querySelector("h1")).not.toBeNull());
    expect(app.calls.filter((call) => call.method === "pad.write")).toHaveLength(0);
  });
});
