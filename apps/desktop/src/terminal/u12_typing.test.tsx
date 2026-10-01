import { cleanup, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import { callsTo, event, info, mountPane, node, shell } from "./paneHarness";

const bytes = (...values: number[]) => Uint8Array.from(values);

afterEach(cleanup);

describe("u12 typing", () => {
  it("u12_each_keystroke_goes_to_terminal_write_as_base64_in_order", async () => {
    const { app, connected, emulators } = await mountPane([shell("a")], [info("t-a")]);

    connected.rail.select("a");
    emulators.get("t-a")?.type(bytes(108));
    emulators.get("t-a")?.type(bytes(115, 13));

    await vi.waitFor(() =>
      expect(callsTo(app, "terminal.write")).toEqual([
        { id: "t-a", data: "bA==" },
        { id: "t-a", data: "cw0=" },
      ]),
    );
  });

  it("u12_a_write_waits_for_the_one_typed_before_it", async () => {
    const { app, connected, emulators } = await mountPane([shell("a")], [info("t-a")]);
    const releases: (() => void)[] = [];

    connected.rail.select("a");

    app.handlers["terminal.write"] = () => new Promise<null>((resolve) => releases.push(() => resolve(null)));
    emulators.get("t-a")?.type(bytes(1));
    emulators.get("t-a")?.type(bytes(2));

    await vi.waitFor(() => expect(releases).toHaveLength(1));
    releases[0]?.();
    await vi.waitFor(() => expect(releases).toHaveLength(2));
  });

  it("u12_what_is_typed_while_a_write_is_in_flight_goes_as_one_write_in_order", async () => {
    const { app, connected, emulators } = await mountPane([shell("a")], [info("t-a")]);
    const releases: (() => void)[] = [];

    app.handlers["terminal.write"] = () => new Promise<null>((resolve) => releases.push(() => resolve(null)));
    connected.rail.select("a");
    emulators.get("t-a")?.type(bytes(1));
    emulators.get("t-a")?.type(bytes(2));
    emulators.get("t-a")?.type(bytes(3, 4));
    await vi.waitFor(() => expect(releases).toHaveLength(1));
    releases[0]?.();
    await vi.waitFor(() => expect(releases).toHaveLength(2));

    expect(callsTo(app, "terminal.write")).toEqual([
      { id: "t-a", data: "AQ==" },
      { id: "t-a", data: "AgME" },
    ]);
  });

  it("u12_an_exited_terminal_takes_no_input", async () => {
    const { app, connected, emulators } = await mountPane(
      [shell("a"), shell("b")],
      [info("t-a", { running: false, exit_code: 0 }), info("t-b")],
    );

    connected.rail.select("a");
    emulators.get("t-a")?.type(bytes(65));
    connected.rail.select("b");
    emulators.get("t-b")?.type(bytes(66));

    await vi.waitFor(() => expect(callsTo(app, "terminal.write")).toEqual([{ id: "t-b", data: "Qg==" }]));
  });

  it("u12_a_terminal_that_exits_while_shown_stops_taking_input", async () => {
    const { app, connected, emulators } = await mountPane([shell("a"), shell("b")], [info("t-a"), info("t-b")]);

    connected.rail.select("a");
    app.emit(event({ name: "terminal.exited", data: { id: "t-a", code: 1 } }));
    emulators.get("t-a")?.type(bytes(65));
    connected.rail.select("b");
    emulators.get("t-b")?.type(bytes(66));

    await vi.waitFor(() => expect(callsTo(app, "terminal.write")).toEqual([{ id: "t-b", data: "Qg==" }]));
  });

  it("u12_a_failed_write_shows_its_message", async () => {
    const { app, connected, emulators } = await mountPane([node("a")], [info("t-a")]);

    connected.rail.select("a");
    app.handlers["terminal.write"] = () => {
      throw new Error("conflict: write queue full");
    };

    emulators.get("t-a")?.type(bytes(1));

    expect((await screen.findByText("✕ conflict: write queue full")).className).toBe("ink");
  });

  it("u12_a_failed_write_does_not_stall_the_keystrokes_after_it", async () => {
    const { app, connected, emulators } = await mountPane([node("a")], [info("t-a")]);
    let attempts = 0;

    connected.rail.select("a");

    app.handlers["terminal.write"] = () => {
      attempts += 1;

      if (attempts === 1) throw new Error("conflict: write queue full");

      return null;
    };

    emulators.get("t-a")?.type(bytes(1));
    emulators.get("t-a")?.type(bytes(2));

    await vi.waitFor(() => expect(attempts).toBe(2));
  });
});
