import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { RpcError } from "./seam";
import { createFakeApp } from "../testing/fakeApp";
import { node } from "../testing/nodes";
import type { FakeApp } from "../testing/fakeApp";

const REGIONS = ["rail.tree", "terminal.list", "todo.list", "pad.list"] as const;

const openApp = async () => {
  const app = createFakeApp();

  app.opened.project = { name: "payments-api", path: "/work/payments-api" };

  const { container } = render(() => <App app={app} reducedMotion={() => false} clock={() => 0} />);

  await waitFor(() => expect(container.querySelector(".pane")).not.toBeNull());

  return { app, header: screen.getByRole("banner") };
};

const callsOf = (app: FakeApp, method: string) => app.calls.filter((call) => call.method === method).length;

const reopen = (header: HTMLElement) => fireEvent.click(within(header).getByRole("button", { name: "reopen" }));

afterEach(cleanup);

describe("u37 reopen after the Daemon exits", () => {
  it("u37_reopen_shows_after_the_exit_text_and_not_before", async () => {
    const { app, header } = await openApp();

    expect(within(header).queryByRole("button", { name: "reopen" })).toBeNull();

    app.exitDaemon({ code: 1 });

    expect(header.textContent).toBe("roundup   payments-api   daemon exited 1");
    expect(within(header).getByRole("button", { name: "reopen" }).getAttribute("data-label")).toBe("reopen");
  });

  it("u37_clicking_reopen_calls_open_project_with_the_project_path", async () => {
    const { app, header } = await openApp();
    const paths: string[] = [];
    const open = app.openProject;

    app.openProject = (path) => (paths.push(path), open(path));
    app.exitDaemon({ code: 1 });
    reopen(header);

    expect(paths).toEqual(["/work/payments-api"]);
  });

  it("u37_success_restores_the_header_and_enables_the_controls", async () => {
    const { app, header } = await openApp();

    app.exitDaemon({ code: 1 });
    reopen(header);

    await waitFor(() => expect(screen.getByRole("banner").textContent).toBe("roundup   payments-api"));

    for (const word of ["agent", "terminal", "room"]) {
      expect(screen.getByRole("button", { name: word }).hasAttribute("disabled")).toBe(false);
    }
  });

  it("u37_success_refetches_every_region_and_subscribes_again", async () => {
    const { app, header } = await openApp();
    const subscribed = { count: 0 };
    const subscribe = app.subscribe;

    app.subscribe = (listener) => (subscribed.count++, subscribe(listener));

    const before = REGIONS.map((method) => callsOf(app, method));

    app.exitDaemon({ code: 1 });
    reopen(header);

    await waitFor(() => expect(subscribed.count).toBe(1));
    await waitFor(() => REGIONS.forEach((method, index) => expect(callsOf(app, method)).toBeGreaterThan(before[index]!)));
  });

  it("u37_the_rail_shows_the_new_daemons_tree_never_a_mix", async () => {
    const { app, header } = await openApp();

    app.handlers["rail.tree"] = () => [node("old", { name: "old-agent" })];
    app.exitDaemon({ code: 1 });
    app.handlers["rail.tree"] = () => [node("new", { name: "new-agent" })];
    reopen(header);

    await waitFor(() => expect(screen.queryByText("new-agent")).not.toBeNull());

    expect(screen.queryByText("old-agent")).toBeNull();
  });

  it("u37_a_failed_reopen_shows_the_message_in_ink_and_offers_reopen_again", async () => {
    const { app, header } = await openApp();

    app.exitDaemon({ code: 1 });
    app.opened.failure = new RpcError(-32603, "rupd stayed silent");
    reopen(header);

    await waitFor(() => expect(within(header).queryByText("rupd stayed silent")).not.toBeNull());

    expect(within(header).getByText("rupd stayed silent").className).toBe("ink");
    expect(within(header).queryByText("daemon exited 1")).toBeNull();
    expect(within(header).getByRole("button", { name: "reopen" })).not.toBeNull();
  });

  it("u37_a_reopen_after_a_failed_one_can_succeed", async () => {
    const { app, header } = await openApp();

    app.exitDaemon({ code: 1 });
    app.opened.failure = new RpcError(-32603, "rupd stayed silent");
    reopen(header);
    await waitFor(() => expect(within(header).queryByText("rupd stayed silent")).not.toBeNull());

    app.opened.failure = null;
    reopen(header);

    await waitFor(() => expect(screen.getByRole("banner").textContent).toBe("roundup   payments-api"));
  });
});
