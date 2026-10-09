import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import { callsTo } from "../terminal/paneHarness";
import { agent, door, USER } from "../testing/nodes";
import { message, mountThread, sent, threadInput } from "./threadFixture";
import { event } from "../testing/nodes";

const tree = () => [door("room", "idle", "idle", { name: "lead" }), agent("a", "working", "x", { parent: "room", name: "ann" }), agent("b", "idle", "x", { parent: "room", name: "bob" })];

const feedLines = () => [...document.querySelectorAll(".thread-line")].map((line) => line.textContent);

const ctrlO = (target: Element) => fireEvent.keyDown(target, { key: "o", code: "KeyO", ctrlKey: true });

afterEach(cleanup);

describe("u106 agent-to-agent Messages are one line", () => {
  it("u106_three_messages_show_one_folded_line_each_and_no_body", async () => {
    const long = "x".repeat(80);

    const { connected } = await mountThread(tree(), {
      messages: [message(1, { body: "short body" }), message(2, { body: long }), message(3, { kind: "question", body: "why?" })],
    });

    await vi.waitFor(() => expect(feedLines()).toHaveLength(3));

    expect(feedLines()).toEqual(["ann → bob note short body…", `ann → bob note ${"x".repeat(60)}…`, "ann → bob question why?…"]);
    expect(connected.rail.nodes).toHaveLength(3);
  });

  it("u106_a_long_body_is_cut_at_60_characters", async () => {
    await mountThread(tree(), { messages: [message(1, { body: `${"a".repeat(59)}bc` })] });

    await vi.waitFor(() => expect(feedLines()[0]).toBe(`ann → bob note ${"a".repeat(59)}b…`));
  });

  it("u106_ctrl_o_in_the_input_or_feed_expands_every_line_and_again_folds_them", async () => {
    const body = "line one\nline two that is longer than sixty characters ".repeat(2);
    await mountThread(tree(), { messages: [message(1, { body })] });
    await vi.waitFor(() => expect(feedLines()).toHaveLength(1));

    ctrlO(threadInput());

    expect(feedLines()[0]).toBe(`ann → bob note ${body}`);

    ctrlO(screen.getByRole("log"));

    expect(feedLines()[0]).toMatch(/…$/);
  });

  it("u106_a_message_to_the_user_is_not_folded", async () => {
    const { app } = await mountThread(tree());

    app.emit(sent(message(5, { to: "you", body: "z".repeat(90) })));
    app.emit(event({ name: "message.sent", data: message(6, { from: USER, to: "room", body: "q".repeat(90) }) }));

    await vi.waitFor(() => expect(feedLines()).toHaveLength(2));

    expect(feedLines()).toEqual([`ann → you note ${"z".repeat(90)}`, `you → lead note ${"q".repeat(90)}`]);
  });

  it("u106_ctrl_o_in_the_pane_reaches_terminal_write_and_does_not_toggle", async () => {
    const { app, connected, emulators } = await mountThread(tree(), { messages: [message(1)] });
    await vi.waitFor(() => expect(feedLines()).toHaveLength(1));

    connected.rail.select("a");
    ctrlO(document.querySelector(".pane-screen [data-terminal]")!);
    emulators.get("t-a")?.type(Uint8Array.of(15));

    expect(feedLines()[0]).toMatch(/…$/);
    await vi.waitFor(() => expect(callsTo(app, "terminal.write")).toEqual([{ id: "t-a", data: "Dw==" }]));
  });

  it("u106_the_chord_is_not_kept_in_storage", async () => {
    await mountThread(tree(), { messages: [message(1)] });
    ctrlO(threadInput());

    expect(JSON.stringify({ ...localStorage })).not.toMatch(/expanded|ctrl/i);
  });
});
