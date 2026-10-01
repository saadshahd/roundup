import { cleanup, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { event, info, mountPane, node, output, shell } from "./paneHarness";

const decoded = (chunks: Uint8Array[]): string[] => chunks.map((chunk) => new TextDecoder().decode(chunk));

afterEach(cleanup);

describe("u11 one emulator per Terminal", () => {
  it("u11_output_is_decoded_and_handed_to_the_emulator_of_its_terminal_in_order", async () => {
    const { app, emulators } = await mountPane([node("a"), node("b")], [info("t-a"), info("t-b")]);

    app.emit(output("t-a", "one"));
    app.emit(output("t-b", "elsewhere"));
    app.emit(output("t-a", "two"));

    expect([decoded(emulators.get("t-a")?.written ?? []), decoded(emulators.get("t-b")?.written ?? [])]).toEqual([
      ["one", "two"],
      ["elsewhere"],
    ]);
  });

  it("u11_output_for_a_terminal_that_is_not_selected_still_creates_its_emulator", async () => {
    const { app, emulators } = await mountPane([node("a")], [info("t-a")]);

    app.emit(output("t-a", "early"));

    expect(decoded(emulators.get("t-a")?.written ?? [])).toEqual(["early"]);
  });

  it("u11_selecting_a_row_shows_its_emulator_and_no_other", async () => {
    const { app, connected, emulators, container } = await mountPane([node("a"), node("b")], [info("t-a"), info("t-b")]);

    app.emit(output("t-a", "x"));
    app.emit(output("t-b", "y"));
    connected.rail.select("b");

    expect([emulators.get("t-b")?.host?.textContent, container.querySelector(".pane-screen")?.textContent]).toEqual([
      "t-b",
      "t-b",
    ]);
  });

  it("u11_selecting_a_group_shows_no_emulator", async () => {
    const { connected, container } = await mountPane([node("a"), node("g", { kind: "group", status: null, terminal_id: null })]);

    connected.rail.select("a");
    connected.rail.select("g");

    expect(container.querySelector(".pane-screen")?.textContent).toBe("");
  });

  it("u11_bad_base64_shows_one_error_line_and_later_output_still_arrives", async () => {
    const { app, emulators } = await mountPane([node("a")], [info("t-a")]);

    app.emit(event({ name: "terminal.output", data: { id: "t-a", data: "***" } }));
    app.emit(output("t-a", "after"));

    expect([screen.getByText(/^✕ terminal\.output:/).tagName, decoded(emulators.get("t-a")?.written ?? [])]).toEqual([
      "P",
      ["after"],
    ]);
  });

  it("u11_closing_the_pane_disposes_every_emulator", async () => {
    const { app, emulators } = await mountPane([shell("a")], [info("t-a")]);

    app.emit(output("t-a", "x"));
    cleanup();

    expect(emulators.get("t-a")?.disposed).toBe(true);
  });
});
