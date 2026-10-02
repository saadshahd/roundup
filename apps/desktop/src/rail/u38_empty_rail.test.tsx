import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { RpcError } from "../app/seam";
import { connectProject, ConnectedProjectContext } from "../state/connectedProject";
import { createFakeApp } from "../testing/fakeApp";
import { mountRail } from "./railFixture";
import { node } from "../testing/nodes";
import { Rail } from "./Rail";

afterEach(cleanup);

describe("u38 empty Rail", () => {
  it("u38_an_empty_rail_reads_no_agents_yet_and_cmd_n_starts_one", async () => {
    await mountRail([]);

    expect([screen.getByText("no agents yet"), screen.getByText("⌘N starts one")].length).toBe(2);
  });

  it("u38_the_empty_rail_lines_disappear_once_a_row_exists", async () => {
    await mountRail([node("a")]);

    expect([screen.queryByText("no agents yet"), screen.queryByText("⌘N starts one")]).toEqual([null, null]);
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
