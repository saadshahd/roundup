import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { RpcError } from "../app/seam";
import { connectProject, ConnectedProjectContext } from "../state/connectedProject";
import { createFakeApp } from "../testing/fakeApp";
import { node } from "../testing/nodes";
import { mountPane } from "./paneHarness";
import { Pane } from "./Pane";

afterEach(cleanup);

describe("u38 empty Pane", () => {
  it("u38_a_selected_agent_shows_no_empty_pane_text", async () => {
    const { connected } = await mountPane([node("a")]);

    connected.rail.select("a");

    expect(screen.queryByText("select an agent or a terminal")).toBeNull();
  });

  it("u38_a_failed_initial_rail_tree_shows_no_empty_pane_text", async () => {
    const app = createFakeApp();

    app.handlers["rail.tree"] = () => {
      throw new RpcError(-32603, "daemon is gone");
    };

    const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, () => 0);

    render(() => (
      <ConnectedProjectContext.Provider value={connected}>
        <Pane notice={connected.rail.failure()} />
      </ConnectedProjectContext.Provider>
    ));

    expect(screen.queryByText("select an agent or a terminal")).toBeNull();
  });
});
