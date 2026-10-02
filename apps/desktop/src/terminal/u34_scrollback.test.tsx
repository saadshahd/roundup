import { Terminal } from "@xterm/xterm";
import { cleanup, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { SCROLLBACK_LINES, xtermOptions } from "./emulator";
import { mountPane, output } from "./paneHarness";
import { info, node, terminal } from "../testing/nodes";

afterEach(cleanup);

describe("u34 bounded scrollback", () => {
  it("u34_the_real_emulator_is_constructed_with_a_bound_of_10_000_lines", () => {
    expect([SCROLLBACK_LINES, new Terminal(xtermOptions).options.scrollback]).toEqual([10_000, 10_000]);
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

  it("u34_a_selected_group_has_no_latest_control", async () => {
    const { connected } = await mountPane([node("g", { kind: "group", status: null, terminal_id: null })]);

    connected.rail.select("g");

    expect(screen.queryByText("↓ latest")).toBeNull();
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
