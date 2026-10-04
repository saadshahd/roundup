import { cleanup, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import type { Pad } from "@contracts/pad/Pad";
import { connectProject, ConnectedProjectContext } from "../state/connectedProject";
import { createFakeApp } from "../testing/fakeApp";
import { Pads } from "./Pads";
import { deferred, openShelf, padOf, AGENT } from "./padsFixture";

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
    const { app, state } = await openShelf([]);

    await screen.findByText("no pads yet");

    state.pads = [padOf("auth-notes", AGENT)];
    app.emit({ actor: AGENT, name: "pad.changed", data: { name: "auth-notes" } });

    await screen.findByText("auth-notes");
    expect(screen.queryByText("no pads yet")).toBeNull();
  });

  it("u38_a_stale_first_pad_list_reply_never_sets_no_pads_yet_before_the_newest_one_lands", async () => {
    const app = createFakeApp();
    const replies: ReturnType<typeof deferred<Pad[]>>[] = [];

    app.handlers["pad.list"] = () => {
      const reply = deferred<Pad[]>();

      replies.push(reply);

      return reply.promise;
    };

    const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, () => 0);

    render(() => (
      <ConnectedProjectContext.Provider value={connected}>
        <Pads />
      </ConnectedProjectContext.Provider>
    ));

    await waitFor(() => expect(replies.length).toBe(1));

    app.emit({ actor: AGENT, name: "pad.changed", data: { name: "auth-notes" } });
    await waitFor(() => expect(replies.length).toBe(2));

    replies[0]?.resolve([]);
    await new Promise((done) => setTimeout(done, 0));

    expect(screen.queryByText("no pads yet")).toBeNull();

    replies[1]?.resolve([padOf("auth-notes", AGENT)]);

    expect(await screen.findByText("auth-notes")).toBeTruthy();
    expect(screen.queryByText("no pads yet")).toBeNull();
  });

  it("u38_a_stale_pad_list_failure_after_a_newer_empty_reply_leaves_no_pads_yet_alone", async () => {
    const app = createFakeApp();
    const replies: ReturnType<typeof deferred<Pad[]>>[] = [];

    app.handlers["pad.list"] = () => {
      const reply = deferred<Pad[]>();

      replies.push(reply);

      return reply.promise;
    };

    const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, () => 0);

    render(() => (
      <ConnectedProjectContext.Provider value={connected}>
        <Pads />
      </ConnectedProjectContext.Provider>
    ));

    await waitFor(() => expect(replies.length).toBe(1));

    app.emit({ actor: AGENT, name: "pad.changed", data: { name: "auth-notes" } });
    await waitFor(() => expect(replies.length).toBe(2));

    replies[1]?.resolve([]);
    expect(await screen.findByText("no pads yet")).toBeTruthy();

    replies[0]?.reject(new Error("stale boom"));
    await new Promise((done) => setTimeout(done, 0));

    expect(screen.getByText("no pads yet")).toBeTruthy();
    expect(screen.queryByText(/stale boom/)).toBeNull();
  });

  it("u38_no_pads_yet_returns_once_the_last_pad_is_removed", async () => {
    const { app, state } = await openShelf([padOf("auth-notes", AGENT)]);

    await screen.findByText("auth-notes");

    state.pads = [];
    app.emit({ actor: AGENT, name: "pad.changed", data: { name: "auth-notes" } });

    expect(await screen.findByText("no pads yet")).toBeTruthy();
    expect(screen.queryByText("auth-notes")).toBeNull();
  });

  it("u38_a_retry_after_a_failed_pad_list_keeps_the_failure_until_the_reply_arrives", async () => {
    const app = createFakeApp();
    const retry = deferred<Pad[]>();
    let calls = 0;

    app.handlers["pad.list"] = () => {
      calls += 1;

      return calls === 1 ? Promise.reject(new Error("boom")) : retry.promise;
    };

    const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, () => 0);

    render(() => (
      <ConnectedProjectContext.Provider value={connected}>
        <Pads />
      </ConnectedProjectContext.Provider>
    ));

    expect((await screen.findByText(/boom/)).textContent).toBe("boom");

    app.emit({ actor: AGENT, name: "pad.changed", data: { name: "auth-notes" } });
    await waitFor(() => expect(calls).toBe(2));

    expect(screen.getByText("boom")).toBeTruthy();
    expect(screen.queryByText("no pads yet")).toBeNull();

    retry.resolve([]);

    expect(await screen.findByText("no pads yet")).toBeTruthy();
    expect(screen.queryByText(/boom/)).toBeNull();
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

    expect((await screen.findByText(/daemon says no/)).textContent).toBe("daemon says no");
    expect(screen.queryByText("no pads yet")).toBeNull();
  });
});
