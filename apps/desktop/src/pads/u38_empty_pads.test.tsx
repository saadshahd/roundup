import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { connectProject, ConnectedProjectContext } from "../state/connectedProject";
import { createFakeApp } from "../testing/fakeApp";
import { Pads } from "./Pads";
import { openShelf, padOf, AGENT } from "./padsFixture";

afterEach(cleanup);

describe("u38 empty Pads", () => {
  it("u38_an_open_project_with_no_pads_reads_no_pads_yet", async () => {
    await openShelf([]);

    expect(await screen.findByText("no pads yet")).toBeTruthy();
  });

  it("u38_no_pads_yet_never_flashes_before_the_first_pad_list_answers", async () => {
    const app = createFakeApp();
    const pending = Promise.withResolvers<ReturnType<typeof padOf>[]>();

    app.handlers["pad.list"] = () => pending.promise;

    const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, () => 0);

    render(() => (
      <ConnectedProjectContext.Provider value={connected}>
        <Pads />
      </ConnectedProjectContext.Provider>
    ));

    expect(screen.queryByText("no pads yet")).toBeNull();

    pending.resolve([]);

    expect(await screen.findByText("no pads yet")).toBeTruthy();
  });

  it("u38_no_pads_yet_disappears_once_a_pad_exists", async () => {
    await openShelf([padOf("auth-notes", AGENT)]);
    await screen.findByText("auth-notes");

    expect(screen.queryByText("no pads yet")).toBeNull();
  });

  it("u38_a_failed_initial_pad_list_shows_its_message_not_no_pads_yet", async () => {
    const app = createFakeApp();

    app.handlers["pad.list"] = () => {
      throw new Error("daemon says no");
    };

    const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, () => 0);

    render(() => (
      <ConnectedProjectContext.Provider value={connected}>
        <Pads />
      </ConnectedProjectContext.Provider>
    ));

    expect((await screen.findByText(/daemon says no/)).textContent).toBe("✕ daemon says no");
    expect(screen.queryByText("no pads yet")).toBeNull();
  });
});
