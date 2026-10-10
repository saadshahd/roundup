import { cleanup, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { event, info, node, terminal } from "../testing/nodes";
import { callsTo, mountPane, output } from "./paneHarness";

afterEach(cleanup);

describe("u57 no pane header", () => {
  it.each([
    ["Agent", node("a", { name: "auth-refactor", status: { kind: "working", label: "editing", since: 0 } })],
    ["Door", node("a", { kind: "workstream", attempt: "1" })],
    ["Terminal", terminal("a", { name: "npm run dev" })],
  ])("u57_a_selected_%s_shows_only_its_terminal_from_the_top", async (_kind, selected) => {
    const { connected, container, app } = await mountPane([selected], [info("t-a")], 12 * 60_000);

    connected.rail.select("a");

    const pane = container.querySelector(".pane");
    const body = container.querySelector(".pane-body");

    expect(pane?.textContent).toBe("t-a");
    expect(pane?.firstElementChild).toBe(body);
    expect(body?.firstElementChild).toBe(container.querySelector(".pane-screen"));
    expect(callsTo(app, "terminal.resize")).toEqual([{ id: "t-a", cols: 100, rows: 30 }]);
  });

  it.each([0, 3, null])("u57_an_exited_terminal_keeps_its_last_output_without_a_header_for_code_%s", async (code) => {
    const { connected, container, app, emulators } = await mountPane([terminal("a")], [info("t-a")]);

    app.emit(output("t-a", "last output"));
    connected.rail.select("a");
    const shown = emulators.get("t-a");
    app.emit(event({ name: "terminal.exited", data: { id: "t-a", code } }));

    expect(container.querySelector(".pane")?.textContent).toBe("t-a");
    expect(emulators.get("t-a")).toBe(shown);
    await waitFor(() => expect(shown?.written.map((bytes) => new TextDecoder().decode(bytes))).toEqual(["last output"]));
    expect(shown?.host).toBe(container.querySelector(".pane-screen"));
    expect(shown?.disposed).toBe(false);
  });

  it("u57_a_status_change_does_not_add_header_text_or_refit_the_terminal", async () => {
    const { connected, container, app } = await mountPane([node("a")], [info("t-a")]);

    connected.rail.select("a");
    app.emit(event({ name: "agent.status", data: { status_revision: "2", attempt: "1", id: "a", status: { kind: "needs-you", label: "asks", since: 0 } } }));

    expect(container.querySelector(".pane")?.textContent).toBe("t-a");
    expect(callsTo(app, "terminal.resize")).toHaveLength(1);
  });
});
