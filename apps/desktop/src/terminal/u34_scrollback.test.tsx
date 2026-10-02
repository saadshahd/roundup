import * as xtermModule from "@xterm/xterm";
import { Terminal } from "@xterm/xterm";
import { cleanup, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SCROLLBACK_LINES, createXtermEmulators, xtermOptions } from "./emulator";
import { mountPane, output } from "./paneHarness";
import { group, info, node, terminal } from "../testing/nodes";

const decoded = (chunks: Uint8Array[]): string[] => chunks.map((chunk) => new TextDecoder().decode(chunk));

const write = (terminal: Terminal, data: string) => new Promise<void>((resolve) => terminal.write(data, resolve));

afterEach(cleanup);

describe("u34 bounded scrollback", () => {
  it("u34_the_real_emulator_is_constructed_with_a_bound_of_10_000_lines", () => {
    expect([SCROLLBACK_LINES, new Terminal(xtermOptions).options.scrollback]).toEqual([10_000, 10_000]);
  });

  it("u34_the_factory_builds_its_real_emulator_with_xtermOptions", () => {
    const spy = vi.spyOn(xtermModule, "Terminal");

    createXtermEmulators()("t-a");

    const calls = spy.mock.calls;

    spy.mockRestore();

    expect(calls).toEqual([[xtermOptions]]);
  });

  it("u34_the_real_emulator_reports_the_underlying_terminals_scroll_state", async () => {
    const spy = vi.spyOn(xtermModule, "Terminal");
    const emulator = createXtermEmulators()("t-a");
    // SAFETY: the one Terminal the spy observed is the instance createXtermEmulators just constructed.
    const terminal = spy.mock.instances[0] as Terminal;

    spy.mockRestore();

    for (let line = 0; line < 50; line++) await write(terminal, `line ${line}\r\n`);

    const atBottomAfterOutput = emulator.isAtBottom();

    let scrolls = 0;

    emulator.onScroll(() => {
      scrolls += 1;
    });
    terminal.scrollToTop();

    const afterScrollingUp = [emulator.isAtBottom(), scrolls];

    emulator.scrollToBottom();

    const afterReturning = [emulator.isAtBottom(), scrolls];

    expect([atBottomAfterOutput, afterScrollingUp, afterReturning]).toEqual([true, [false, 1], [true, 2]]);
  });
});

describe("u34 output regardless of selection (U11)", () => {
  it("u34_output_is_still_handed_to_the_emulator_in_order_whether_or_not_its_row_is_selected", async () => {
    const { app, emulators } = await mountPane([node("a"), node("b")], [info("t-a"), info("t-b")]);

    app.emit(output("t-a", "one"));
    app.emit(output("t-b", "elsewhere"));
    app.emit(output("t-a", "two"));

    expect([decoded(emulators.get("t-a")?.written ?? []), decoded(emulators.get("t-b")?.written ?? [])]).toEqual([
      ["one", "two"],
      ["elsewhere"],
    ]);
  });
});

describe("u34 the latest control", () => {
  it("u34_is_hidden_while_the_view_is_at_the_bottom", async () => {
    const { connected } = await mountPane([node("a")], [info("t-a")]);

    connected.rail.select("a");

    expect(screen.queryByText("↓ latest")).toBeNull();
  });

  it("u34_shows_once_the_user_scrolls_up", async () => {
    const { connected, emulators } = await mountPane([node("a")], [info("t-a")]);

    connected.rail.select("a");
    emulators.get("t-a")?.scroll(false);

    expect(screen.queryByText("↓ latest")).not.toBeNull();
  });

  it("u34_a_selected_group_has_no_latest_control_and_creates_no_emulator", async () => {
    const { connected, emulators } = await mountPane([group("g")]);

    connected.rail.select("g");

    expect([screen.queryByText("↓ latest"), emulators.size]).toEqual([null, 0]);
  });

  it("u34_clicking_it_returns_to_the_newest_line_and_hides_it", async () => {
    const { connected, emulators } = await mountPane([node("a")], [info("t-a")]);

    connected.rail.select("a");
    emulators.get("t-a")?.scroll(false);
    screen.getByText("↓ latest").click();

    expect([emulators.get("t-a")?.scrollsToBottom, screen.queryByText("↓ latest")]).toEqual([1, null]);
  });

  it("u34_typing_returns_to_the_newest_line_and_hides_it", async () => {
    const { connected, emulators } = await mountPane([terminal("a")], [info("t-a")]);

    connected.rail.select("a");
    emulators.get("t-a")?.scroll(false);
    emulators.get("t-a")?.type(Uint8Array.of(108));

    expect([emulators.get("t-a")?.scrollsToBottom, screen.queryByText("↓ latest")]).toEqual([1, null]);
  });

  it("u34_an_exited_terminal_does_not_scroll_on_input", async () => {
    const { connected, emulators } = await mountPane([terminal("a")], [info("t-a", { running: false, exit_code: 0 })]);

    connected.rail.select("a");
    emulators.get("t-a")?.scroll(false);
    emulators.get("t-a")?.type(Uint8Array.of(108));

    expect([emulators.get("t-a")?.scrollsToBottom, screen.queryByText("↓ latest") !== null]).toEqual([0, true]);
  });

  it("u34_output_while_scrolled_up_never_moves_the_view", async () => {
    const { app, connected, emulators } = await mountPane([node("a")], [info("t-a")]);

    connected.rail.select("a");
    emulators.get("t-a")?.scroll(false);
    app.emit(output("t-a", "more"));

    expect([emulators.get("t-a")?.scrollsToBottom, screen.queryByText("↓ latest") !== null]).toEqual([0, true]);
  });
});
