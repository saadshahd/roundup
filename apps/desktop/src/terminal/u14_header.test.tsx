import { cleanup, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import pane from "./styles.css?inline";
import { event, info, mountPane, node, shell } from "./paneHarness";

const MINUTE = 60_000;

const headerOf = (container: HTMLElement) => container.querySelector(".pane-title")?.textContent ?? "";

afterEach(cleanup);

describe("u14 pane header", () => {
  it("u14_an_agent_header_is_its_name_kind_and_elapsed_time", async () => {
    const status = { kind: "working", label: "editing", since: 0 } as const;

    const { connected, container } = await mountPane(
      [node("auth-refactor", { name: "auth-refactor", status })],
      [info("t-auth-refactor")],
      12 * MINUTE,
    );

    connected.rail.select("auth-refactor");

    expect(headerOf(container)).toBe("auth-refactor  working 12m");
  });

  it("u14_a_running_terminal_header_is_its_name_alone", async () => {
    const { connected, container } = await mountPane([shell("dev", { name: "npm run dev" })], [info("t-dev")]);

    connected.rail.select("dev");

    expect(headerOf(container)).toBe("npm run dev");
  });

  it.each([
    ["a known code", { running: false, exit_code: 3 }, "exited 3"],
    ["a signal", { running: false, exit_code: null }, "exited by signal"],
  ])("u14_an_exited_terminal_header_gives_%s", async (_case, exit, wording) => {
    const { connected, container } = await mountPane([shell("dev", { name: "npm run dev" })], [info("t-dev", exit)]);

    connected.rail.select("dev");

    expect(headerOf(container)).toBe(`npm run dev  ${wording}`);
  });

  it("u14_a_terminal_row_with_no_terminal_id_reads_exited_alone", async () => {
    const { connected, container } = await mountPane([shell("dev", { name: "npm run dev", terminal_id: null })]);

    connected.rail.select("dev");

    expect(headerOf(container)).toBe("npm run dev  exited");
  });

  it("u14_a_terminal_that_exits_while_selected_changes_its_header", async () => {
    const { app, connected, container } = await mountPane([shell("dev", { name: "npm run dev" })], [info("t-dev")]);

    connected.rail.select("dev");
    app.emit(event({ name: "terminal.exited", data: { id: "t-dev", code: 0 } }));

    expect(headerOf(container)).toBe("npm run dev  exited 0");
  });

  it("u14_a_status_change_updates_the_agent_header", async () => {
    const { app, connected, container } = await mountPane([node("a", { name: "a" })], [info("t-a")]);

    connected.rail.select("a");
    app.emit(event({ name: "agent.status", data: { id: "a", status: { kind: "needs-you", label: "asks", since: 0 } } }));

    expect(headerOf(container)).toBe("a  needs-you just now");
  });

  it("u14_a_group_has_no_header", async () => {
    const { connected, container } = await mountPane([node("g", { kind: "group", status: null, terminal_id: null })]);

    connected.rail.select("g");

    expect([headerOf(container), screen.queryByText("stop")]).toEqual(["", null]);
  });

  it("u14_with_nothing_selected_the_pane_is_empty", async () => {
    const { container } = await mountPane([node("a")], [info("t-a")]);

    expect([headerOf(container), container.querySelector(".pane-screen")?.textContent, screen.queryByText("stop")]).toEqual([
      "",
      "",
      null,
    ]);
  });

  it("u14_stop_on_a_running_agent_calls_agent_stop", async () => {
    const { app, connected } = await mountPane([node("a")], [info("t-a")]);

    connected.rail.select("a");
    screen.getByText("stop").click();

    expect(app.calls.filter((call) => call.method === "agent.stop").map((call) => call.params)).toEqual([{ id: "a" }]);
  });

  it("u14_stop_on_a_running_terminal_kills_its_terminal_id", async () => {
    const { app, connected } = await mountPane([shell("dev")], [info("t-dev")]);

    connected.rail.select("dev");
    screen.getByText("stop").click();

    expect(app.calls.filter((call) => call.method === "terminal.kill").map((call) => call.params)).toEqual([{ id: "t-dev" }]);
  });

  it("u14_an_exited_terminal_has_no_stop", async () => {
    const { connected } = await mountPane([shell("dev")], [info("t-dev", { running: false, exit_code: 0 })]);

    connected.rail.select("dev");

    expect(screen.queryByText("stop")).toBeNull();
  });

  it("u14_stop_disappears_when_the_program_exits", async () => {
    const { app, connected } = await mountPane([shell("dev")], [info("t-dev")]);

    connected.rail.select("dev");
    app.emit(event({ name: "terminal.exited", data: { id: "t-dev", code: 0 } }));

    expect(screen.queryByText("stop")).toBeNull();
  });

  it("u14_a_failed_stop_shows_its_message", async () => {
    const { app, connected } = await mountPane([node("a")], [info("t-a")]);

    app.handlers["agent.stop"] = () => {
      throw new Error("not found: agent a");
    };

    connected.rail.select("a");
    screen.getByText("stop").click();

    expect((await screen.findByText("✕ not found: agent a")).className).toBe("ink");
  });

  it("u14_an_error_line_never_changes_the_height_of_the_slot_that_holds_it", async () => {
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

    app.handlers["agent.stop"] = () => {
      throw new Error("boom");
    };

    screen.getByText("stop").click();
    await screen.findByText("✕ boom");

    const after = box();

    sheet.remove();

    expect([before, after]).toEqual([["24px", "0"], ["24px", "0"]]);
  });
});
