import { cleanup, fireEvent } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RpcError } from "../app/seam";
import { mountThread, threadInput } from "../thread/threadFixture";
import { agent, door, event, terminal } from "../testing/nodes";

const tree = () => [
  door("workstream", "idle", "idle", { name: "lead" }),
  agent("a", "working", "x", { parent: "workstream", name: "ann" }),
  terminal("sh", { name: "shell", parent: "workstream" }),
  agent("b", "working", "x", { parent: "workstream", name: "bob" }),
];

const alt = (digit: number, target: Element = document.body) => fireEvent.keyDown(target, { key: "¡", code: `Digit${digit}`, altKey: true });

// SAFETY: the filter keeps `takeover.*` calls, whose params are `TakeoverParams`.
const takeoverCalls = (app: { calls: { method: string; params: unknown }[] }) =>
  app.calls.filter((call) => call.method.startsWith("takeover.")).map((call) => `${call.method} ${(call.params as { agent: string }).agent}`);

afterEach(cleanup);

describe("u107 jump to an Agent", () => {
  it("u107_alt_n_selects_the_nth_agent_row_skipping_terminals_and_begins_a_takeover", async () => {
    const { app, connected } = await mountThread(tree());

    alt(3);

    expect(connected.rail.selected()).toBe("b");
    await vi.waitFor(() => expect(takeoverCalls(app)).toEqual(["takeover.begin b"]));
  });

  it("u107_the_door_row_counts_as_the_first_agent", async () => {
    const { app, connected } = await mountThread(tree());

    alt(1);

    expect(connected.rail.selected()).toBe("workstream");
    await vi.waitFor(() => expect(takeoverCalls(app)).toEqual(["takeover.begin workstream"]));
  });

  it("u107_a_second_jump_ends_the_first_takeover_before_the_next_begins", async () => {
    const { app } = await mountThread(tree());

    alt(2);
    await vi.waitFor(() => expect(takeoverCalls(app)).toEqual(["takeover.begin a"]));
    alt(3);

    await vi.waitFor(() => expect(takeoverCalls(app)).toEqual(["takeover.begin a", "takeover.end a", "takeover.begin b"]));
  });

  it("u107_alt_0_ends_the_takeover_and_returns_focus_to_the_threads_input", async () => {
    const { app } = await mountThread(tree());

    alt(2);
    await vi.waitFor(() => expect(takeoverCalls(app)).toEqual(["takeover.begin a"]));
    alt(0);

    await vi.waitFor(() => expect(takeoverCalls(app)).toEqual(["takeover.begin a", "takeover.end a"]));
    expect(document.activeElement).toBe(threadInput());
  });

  it("u107_a_number_with_no_agent_does_nothing", async () => {
    const { app, connected } = await mountThread(tree(), { select: null });

    alt(9);

    expect(connected.rail.selected()).toBeNull();
    expect(takeoverCalls(app)).toEqual([]);
  });

  it("u107_a_conflict_still_moves_the_selection", async () => {
    const { app, connected } = await mountThread(tree());

    app.handlers["takeover.begin"] = () => {
      throw new RpcError(-32002, "agent is done");
    };

    alt(2);

    expect(connected.rail.selected()).toBe("a");
    await vi.waitFor(() => expect(takeoverCalls(app)).toEqual(["takeover.begin a"]));
  });

  // Narrows `u12_each_keystroke_goes_to_terminal_write_as_base64_in_order`: every keystroke but these chords still goes to the Terminal.
  it("u107_the_chord_does_not_reach_terminal_write_from_a_pane_and_narrows_u12", async () => {
    const { app, emulators, connected } = await mountThread(tree());

    connected.rail.select("a");
    const screen = document.querySelector<HTMLElement>(".pane-screen [data-terminal]")!;
    const reached = vi.fn();

    screen.addEventListener("keydown", reached);
    alt(3, screen);

    expect(reached).not.toHaveBeenCalled();
    expect(emulators.get("t-a")?.written).toEqual([]);
    expect(app.calls.filter((call) => call.method === "terminal.write")).toEqual([]);
  });

  it("u107_a_takeover_the_daemon_ended_is_not_ended_again", async () => {
    const { app } = await mountThread(tree());

    alt(2);
    await vi.waitFor(() => expect(takeoverCalls(app)).toEqual(["takeover.begin a"]));
    app.emit(event({ name: "takeover.changed", data: { agent: "a", on: false } }));
    alt(0);
    alt(3);

    await vi.waitFor(() => expect(takeoverCalls(app)).toEqual(["takeover.begin a", "takeover.begin b"]));
  });
});
