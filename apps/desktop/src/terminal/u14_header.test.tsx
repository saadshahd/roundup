import { cleanup, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import pane from "./styles.css?inline";
import { mountPane } from "./paneHarness";
import { event, info, node } from "../testing/nodes";

const headerOf = (container: HTMLElement) => container.querySelector(".pane-title")?.textContent ?? "";

afterEach(cleanup);

describe("pane empty states and error sizing", () => {
  it("u14_a_group_has_no_header", async () => {
    const { connected, container } = await mountPane([node("g", { kind: "group", status: null, terminal_id: null })]);

    connected.rail.select("g");

    expect([headerOf(container), screen.queryByText("stop")]).toEqual(["", null]);
  });

  it("u38_with_nothing_selected_the_pane_reads_select_an_agent_or_a_terminal", async () => {
    const { container } = await mountPane([node("a")], [info("t-a")]);

    expect([headerOf(container), container.querySelector(".pane-screen")?.textContent, screen.queryByText("stop")]).toEqual([
      "",
      "",
      null,
    ]);
    expect(screen.getByText("select an agent or a terminal")).toBeTruthy();
  });

  it("u13_an_error_line_never_changes_the_height_of_the_slot_that_holds_it", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));

    sheet.textContent = pane;

    const { app, connected, container } = await mountPane([node("a")], [info("t-a")]);

    const box = () => {
      const slot = container.querySelector(".pane-failure");

      if (!slot) throw new Error("the pane has no failure slot");

      const style = getComputedStyle(slot);

      return [style.height, style.flexShrink];
    };

    connected.rail.select("a");

    const before = box();

    app.emit(event({ name: "terminal.output", data: { id: "t-a", data: "***" } }));
    await screen.findByText(/^✕ terminal\.output:/);

    const after = box();

    sheet.remove();

    expect([before, after]).toEqual([["24px", "0"], ["24px", "0"]]);
  });
});
