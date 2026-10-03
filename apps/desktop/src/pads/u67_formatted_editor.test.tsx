import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { openShelf, padOf } from "./padsFixture";
import { USER } from "../testing/nodes";

afterEach(cleanup);

const open = async (text: string) => {
  const result = await openShelf([padOf("notes", USER, text)]);
  fireEvent.click(await screen.findByText("notes"));
  await screen.findByLabelText("Pad body");
  fireEvent.click(screen.getByRole("button", { name: "edit" }));

  return result;
};

describe("u67 formatted Pad editing", () => {
  it("u67_edit_opens_formatted_editable_markdown", async () => {
    await open("# Heading\n\n**bold** and *emphasis*");

    const editor = await screen.findByRole("textbox", { name: "Editor" });
    expect(editor.getAttribute("contenteditable")).toBe("true");
    expect(editor.querySelector("h1")?.textContent).toBe("Heading");
    expect(editor.querySelector("strong")?.textContent).toBe("bold");
    expect(editor.textContent).not.toContain("**");
    expect(document.activeElement).toBe(editor);
    expect(screen.getAllByRole("button", { name: "Bold" }).some((button) => button.querySelector("svg"))).toBe(true);
    expect(screen.queryByRole("button", { name: "Image" })).toBeNull();
  });

  it("u67_noop_done_preserves_the_original_source", async () => {
    const source = "# Heading\n\n*emphasis*\n\n";
    const { app } = await open(source);
    await screen.findByRole("textbox", { name: "Editor" });
    fireEvent.click(screen.getByRole("button", { name: "done" }));

    expect(app.calls.filter((call) => call.method === "pad.write")).toHaveLength(0);
    expect((await screen.findByLabelText("Pad body")).querySelector("h1")?.textContent).toBe("Heading");
  });

  it("u67_untrusted_source_has_no_active_html_or_unsafe_link", async () => {
    await open('<img src=x onerror="alert(1)">\n\n[bad](javascript:alert(1))');

    const editor = await screen.findByRole("textbox", { name: "Editor" });
    expect(editor.querySelector("img[onerror], script")).toBeNull();
    expect(editor.querySelector('a[href^="javascript:"]')).toBeNull();
  });
});
