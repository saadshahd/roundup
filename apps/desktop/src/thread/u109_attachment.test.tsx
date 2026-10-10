import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { door } from "../testing/nodes";
import { mountThread, threadInput } from "./threadFixture";

const image = (name: string, bytes = 2048) => new File([new Uint8Array(bytes)], name, { type: "image/png" });

const chips = () => [...document.querySelectorAll(".thread-attachment")];

beforeEach(() => {
  let n = 0;

  URL.createObjectURL = () => `blob:fake-${++n}`;
  URL.revokeObjectURL = () => {};
});

afterEach(cleanup);

describe("u109 an image pasted becomes an Attachment", () => {
  it("u109_a_pasted_image_shows_its_name_and_size", async () => {
    await mountThread([door("workstream", "idle", "idle")]);
    fireEvent.paste(threadInput(), { clipboardData: { files: [image("shot.png", 12 * 1024)] } });

    expect(chips().map((chip) => chip.textContent?.trim())).toEqual(["shot.png 12 KB"]);
  });

  it("u109_a_dropped_image_and_a_second_one_make_two_attachments", async () => {
    await mountThread([door("workstream", "idle", "idle")]);
    const composer = document.querySelector(".thread-composer")!;

    fireEvent.drop(composer, { dataTransfer: { files: [image("one.png", 100)] } });
    fireEvent.drop(composer, { dataTransfer: { files: [image("two.png", 3 * 1024 * 1024)] } });

    expect(chips().map((chip) => chip.textContent?.trim())).toEqual(["one.png 100 B", "two.png 3.0 MB"]);
  });

  it("u109_hover_shows_a_preview_that_leaving_hides", async () => {
    await mountThread([door("workstream", "idle", "idle")]);
    fireEvent.paste(threadInput(), { clipboardData: { files: [image("shot.png")] } });

    fireEvent.mouseEnter(chips()[0]!);

    expect(screen.getByAltText("shot.png").getAttribute("src")).toBe("blob:fake-1");

    fireEvent.mouseLeave(chips()[0]!);

    expect(screen.queryByAltText("shot.png")).toBeNull();
  });

  it("u109_esc_hides_the_preview", async () => {
    await mountThread([door("workstream", "idle", "idle")]);
    fireEvent.paste(threadInput(), { clipboardData: { files: [image("shot.png")] } });
    fireEvent.mouseEnter(chips()[0]!);
    fireEvent.keyDown(document.body, { key: "Escape" });

    expect(screen.queryByAltText("shot.png")).toBeNull();
  });

  it("u109_the_preview_is_at_its_own_size_up_to_480_px", () => {
    const [sheet = ""] = Object.values(import.meta.glob<string>("./styles.css", { query: "?raw", import: "default", eager: true }));

    expect(sheet).toMatch(/\.thread-preview\s*{[^}]*max-width:\s*480px/);
    expect(sheet).not.toMatch(/\.thread-preview\s*{[^}]*[^-]width:\s*\d/);
  });

  it("u109_a_file_that_is_not_an_image_is_refused_with_one_failure_line", async () => {
    await mountThread([door("workstream", "idle", "idle")]);
    fireEvent.drop(document.querySelector(".thread-composer")!, { dataTransfer: { files: [new File(["x"], "notes.txt", { type: "text/plain" })] } });

    expect(chips()).toEqual([]);
    expect(screen.getByRole("alert").textContent).toBe("notes.txt is not an image");
  });

  it("u109_sending_types_nothing_into_a_terminal_and_the_attachment_shows_in_the_feed", async () => {
    const { app } = await mountThread([door("workstream", "idle", "idle")]);
    fireEvent.paste(threadInput(), { clipboardData: { files: [image("shot.png")] } });
    fireEvent.keyDown(threadInput(), { key: "Enter" });

    await vi.waitFor(() => expect(document.querySelectorAll(".thread-feed .thread-attachment")).toHaveLength(1));

    expect(app.calls.filter((call) => call.method === "terminal.write")).toEqual([]);
    expect(app.calls.filter((call) => call.method === "message.send")).toEqual([]);
  });
});
