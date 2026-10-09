import { cleanup, fireEvent } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import { agent, door, terminal } from "../testing/nodes";
import { message, mountThread, threadInput } from "./threadFixture";

const tree = () => [door("room", "idle", "idle", { name: "lead" }), agent("a", "working", "x", { parent: "room", name: "ann" }), terminal("sh", { name: "shell", parent: "room" })];

const quotes = () => [...document.querySelectorAll("[data-quote]")];

const type = (character: string) => {
  fireEvent.keyDown(threadInput(), { key: character });
  threadInput().value += character;
  fireEvent.input(threadInput());
};

afterEach(cleanup);

describe("u110 select-to-quote", () => {
  it("u110_a_terminal_selection_becomes_one_quote_before_the_first_character_only", async () => {
    const { connected, emulators } = await mountThread(tree());

    connected.rail.select("a");
    emulators.get("t-a")!.selectedText = "error: boom";
    type("x");

    expect(quotes()).toHaveLength(1);
    expect(quotes()[0]?.textContent).toContain("ann");
    expect(quotes()[0]?.textContent).toContain("error: boom");
    expect(threadInput().value).toBe("x");

    type("y");

    expect(quotes()).toHaveLength(1);
  });

  it("u110_a_selection_in_a_plain_terminal_names_the_terminal_the_rail_shows", async () => {
    const { connected, emulators } = await mountThread(tree());

    connected.rail.select("sh");
    emulators.get("t-sh")!.selectedText = "ls";
    type("x");

    expect(quotes()[0]?.textContent).toContain("shell");
  });

  it("u110_a_long_selection_is_cut_at_2000_characters_and_marked", async () => {
    const { connected, emulators } = await mountThread(tree());

    connected.rail.select("a");
    emulators.get("t-a")!.selectedText = "z".repeat(2500);
    type("x");

    expect(quotes()[0]?.getAttribute("title")).toBe(`${"z".repeat(2000)} [cut]`);
  });

  it("u110_an_empty_selection_adds_nothing", async () => {
    const { connected } = await mountThread(tree());

    connected.rail.select("a");
    type("x");

    expect(quotes()).toEqual([]);
  });

  it("u110_a_selection_in_the_feed_names_the_message_id", async () => {
    await mountThread(tree(), { messages: [message(7, { to: "room", body: "hello there" })] });
    await vi.waitFor(() => expect(document.querySelector(".thread-line")).not.toBeNull());

    const line = document.querySelector(".thread-line")!;

    document.getSelection()!.selectAllChildren(line);
    document.dispatchEvent(new Event("selectionchange"));
    document.getSelection()!.removeAllRanges();
    type("x");

    expect(quotes()[0]?.textContent).toContain("message 7");
    expect(quotes()[0]?.getAttribute("title")).toBe("ann → lead note hello there…");
  });

  it("u110_a_selection_made_in_the_input_itself_adds_nothing", async () => {
    const { connected, emulators } = await mountThread(tree());

    connected.rail.select("a");
    emulators.get("t-a")!.selectedText = "";
    threadInput().value = "typed";
    threadInput().select();
    document.dispatchEvent(new Event("selectionchange"));
    type("x");

    expect(quotes()).toEqual([]);
  });
});
