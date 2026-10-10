import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import { agent, door, event, USER } from "../testing/nodes";
import { message, mountThread, sent } from "./threadFixture";

const tree = () => [
  door("r1", "idle", "idle", { name: "one" }),
  agent("a1", "working", "x", { parent: "r1", name: "ann" }),
  door("r2", "idle", "idle", { name: "two" }),
  agent("a2", "working", "x", { parent: "r2", name: "bob" }),
];

const lineTexts = () => [...document.querySelectorAll(".thread-line")].map((line) => line.textContent ?? "");

const door2you = (id: number, from: string, body: string) => message(id, { from: { kind: "agent", id: from, parent: null }, to: "you", body });

const messages = () => [
  message(1, { from: USER, to: "r1", body: "to one" }),
  message(2, { from: USER, to: "r2", body: "to two" }),
  door2you(3, "r1", "one answers"),
  door2you(4, "r2", "two answers"),
  message(5, { from: { kind: "agent", id: "a1", parent: "r1" }, to: "a2", body: "across" }),
  message(6, { from: { kind: "agent", id: "a1", parent: "r1" }, to: "r1", body: "within" }),
];

afterEach(cleanup);

describe("u160 the Thread shows only the selected Workstream's Messages", () => {
  it("u160_scope_lists_only_messages_between_this_workstream_and_you_or_itself", async () => {
    await mountThread(tree(), { messages: messages() });
    await vi.waitFor(() => expect(lineTexts().length).toBeGreaterThan(0));

    const text = lineTexts().join("\n");

    expect(text).toContain("to one");
    expect(text).toContain("one answers");
    expect(text).toContain("within");
    expect(text).not.toContain("to two");
    expect(text).not.toContain("two answers");
    expect(text).not.toContain("across");
  });

  it("u160_selecting_another_workstream_swaps_the_feed_and_an_agent_keeps_its_workstreams_feed", async () => {
    const { connected } = await mountThread(tree(), { messages: messages() });

    await vi.waitFor(() => expect(lineTexts().join("\n")).toContain("to one"));
    connected.rail.select("r2");
    await vi.waitFor(() => expect(lineTexts().join("\n")).toContain("two answers"));

    expect(lineTexts().join("\n")).not.toContain("one answers");

    connected.rail.select("a2");

    expect(lineTexts().join("\n")).toContain("two answers");
    expect(lineTexts().join("\n")).not.toContain("to one");
  });

  it("u160_an_event_for_another_workstream_lists_nothing_and_a_status_event_changes_no_line_here", async () => {
    const { app, connected } = await mountThread(tree(), { messages: messages() });

    await vi.waitFor(() => expect(lineTexts().length).toBe(3));

    const before = lineTexts();

    app.emit(sent(message(7, { from: USER, to: "r2", body: "late two" })));
    app.emit(event({ name: "message.delivered", data: message(2, { from: USER, to: "r2", body: "to two" }) }));
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(lineTexts()).toEqual(before);
    expect(screen.queryByText(/late two/)).toBeNull();
    expect(connected.rail.nodes).toHaveLength(4);
  });
});
