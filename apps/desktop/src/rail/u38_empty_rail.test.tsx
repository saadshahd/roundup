import { cleanup, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import type { RailNode } from "@contracts/agent/RailNode";
import { RpcError } from "../app/seam";
import { connectProject, ConnectedProjectContext } from "../state/connectedProject";
import { createFakeApp } from "../testing/fakeApp";
import { mountRail } from "./railFixture";
import { event, node } from "../testing/nodes";
import { Rail } from "./Rail";

afterEach(cleanup);

describe("u38 empty Rail", () => {
  it("u38_an_empty_rail_reads_no_agents_yet_and_cmd_n_starts_one", async () => {
    await mountRail([]);

    expect(screen.getByText("no agents yet")).toBeTruthy();
    expect(screen.getByText("⌘N starts one")).toBeTruthy();
  });

  it("u38_the_empty_rail_lines_disappear_once_a_row_exists", async () => {
    await mountRail([node("a")]);

    expect([screen.queryByText("no agents yet"), screen.queryByText("⌘N starts one")]).toEqual([null, null]);
  });

  it("u38_the_empty_rail_lines_return_once_the_last_row_is_removed", async () => {
    const tree = [node("a")];
    const { app } = await mountRail(tree);

    await screen.findByText("a");

    tree.length = 0;
    app.emit(event({ name: "rail.changed" }));

    expect(await screen.findByText("no agents yet")).toBeTruthy();
    expect(screen.getByText("⌘N starts one")).toBeTruthy();
  });

  it("u38_a_retry_after_a_failed_rail_tree_keeps_the_failure_until_the_reply_arrives", async () => {
    const app = createFakeApp();
    const retry = Promise.withResolvers<RailNode[]>();
    let calls = 0;

    app.handlers["rail.tree"] = () => {
      calls += 1;

      if (calls === 1) throw new RpcError(-32603, "daemon is gone");

      return retry.promise;
    };

    app.handlers["terminal.list"] = () => [];

    const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, () => 0);

    render(() => (
      <ConnectedProjectContext.Provider value={connected}>
        <Rail />
      </ConnectedProjectContext.Provider>
    ));

    // Rail.tsx renders no text of its own for `rail.failure()` (N1: the centre's Pane does, via its `notice` prop), so
    // the failure is read from RailState directly, and only its effect on the empty line is asserted through the DOM.
    expect(connected.rail.failure()).toBe("daemon is gone");
    expect(screen.queryByText("no agents yet")).toBeNull();

    app.emit(event({ name: "rail.changed" }));
    await waitFor(() => expect(calls).toBe(2));

    expect(connected.rail.failure()).toBe("daemon is gone");
    expect(screen.queryByText("no agents yet")).toBeNull();

    retry.resolve([]);
    await waitFor(() => expect(connected.rail.failure()).toBeNull());

    expect(await screen.findByText("no agents yet")).toBeTruthy();
  });

  it("u38_a_failed_initial_rail_tree_shows_no_empty_line", async () => {
    const app = createFakeApp();

    app.handlers["rail.tree"] = () => {
      throw new RpcError(-32603, "daemon is gone");
    };

    const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, () => 0);

    render(() => (
      <ConnectedProjectContext.Provider value={connected}>
        <Rail />
      </ConnectedProjectContext.Provider>
    ));

    expect([screen.queryByText("no agents yet"), screen.queryByText("⌘N starts one")]).toEqual([null, null]);
  });
});
