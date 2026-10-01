import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { createFakeApp } from "../testing/fakeApp";
import type { FakeApp } from "../testing/fakeApp";
import { App } from "../App";
import { RpcError } from "./seam";

const PROJECT = { name: "payments-api", path: "/Users/me/repos/payments-api" };

const renderApp = (app: FakeApp) => {
  return render(() => <App app={app} reducedMotion={() => false} clock={() => 0} />);
};

const headerOf = (container: HTMLElement) => container.querySelector("header")?.textContent;

const region = (name: string) => screen.getByRole("region", { name });

afterEach(cleanup);

describe("u2 first run", () => {
  it("u2_with_no_open_project_the_rail_region_reads_its_empty_text", async () => {
    renderApp(createFakeApp());

    await screen.findByText("open a folder to start");

    expect(region("rail").textContent).toBe("agents and terminalsappear here, one per row,nested by indent");
  });

  it("u2_with_no_open_project_the_centre_reads_open_a_folder_and_choose_folder", async () => {
    renderApp(createFakeApp());

    await screen.findByText("open a folder to start");

    expect(within(region("centre")).getByRole("button").textContent).toBe("choose folder…");
  });

  it("u2_with_no_open_project_the_shelf_reads_todos_and_pads", async () => {
    renderApp(createFakeApp());

    await screen.findByText("open a folder to start");

    expect(region("shelf").textContent).toBe(
      "todosagents add them as theyplan; so can youpads◈ agent notes◇ yours",
    );
  });

  it("u2_with_no_open_project_the_header_reads_only_roundup", async () => {
    const { container } = renderApp(createFakeApp());

    await screen.findByText("open a folder to start");

    expect(headerOf(container)).toBe("roundup");
  });

  it("u2_choosing_a_folder_in_the_chooser_opens_it_and_the_header_names_the_project", async () => {
    const app = createFakeApp();
    app.chooser.path = PROJECT.path;
    const { container } = renderApp(app);

    fireEvent.click(await screen.findByText("choose folder…"));

    await waitFor(() => expect(headerOf(container)).toBe("roundup   payments-api"));
  });

  it("u2_cancelling_the_chooser_leaves_the_first_run_screen", async () => {
    const app = createFakeApp();
    app.chooser.path = null;
    renderApp(app);

    fireEvent.click(await screen.findByText("choose folder…"));

    expect(await screen.findByText("open a folder to start")).toBeTruthy();
  });

  it("u2_an_app_that_already_has_a_project_shows_it_without_a_chooser", async () => {
    const app = createFakeApp();
    app.opened.project = PROJECT;
    const { container } = renderApp(app);


    await waitFor(() => expect(headerOf(container)).toBe("roundup   payments-api"));
  });

  it("u2_an_open_project_replaces_the_empty_texts_in_all_three_regions", async () => {
    const app = createFakeApp();
    app.opened.project = PROJECT;
    const { container } = renderApp(app);

    await waitFor(() => expect(headerOf(container)).toBe("roundup   payments-api"));

    expect(container.textContent).not.toMatch(/agents and terminals|open a folder to start|agents add them as they/);
  });

  it("u2_a_failed_project_call_shows_the_message_in_ink", async () => {
    const app = createFakeApp();
    app.project = () => Promise.reject(new RpcError(-32603, "app is not ready"));
    renderApp(app);

    expect((await screen.findByText("✕ app is not ready")).className).toBe("ink");
  });

  it("u2_a_failed_chooser_shows_the_message_in_ink", async () => {
    const app = createFakeApp();
    app.chooseProjectPath = () => Promise.reject(new Error("the chooser could not open"));
    renderApp(app);

    fireEvent.click(await screen.findByText("choose folder…"));

    expect((await screen.findByText("✕ the chooser could not open")).className).toBe("ink");
  });

  it("u2_a_failed_open_project_shows_the_message_in_ink_in_place_of_the_prompt", async () => {
    const app = createFakeApp();
    app.chooser.path = "/not/a/dir";
    app.opened.failure = new RpcError(-32602, "/not/a/dir is not a directory");
    renderApp(app);

    fireEvent.click(await screen.findByText("choose folder…"));

    expect((await screen.findByText("✕ /not/a/dir is not a directory")).textContent).toBe(
      "✕ /not/a/dir is not a directory",
    );
    expect(screen.queryByText("open a folder to start")).toBeNull();
  });

  it("u2_a_failed_open_project_is_the_only_ink_in_the_window", async () => {
    const app = createFakeApp();
    app.chooser.path = "/not/a/dir";
    app.opened.failure = new RpcError(-32602, "not a directory");
    const { container } = renderApp(app);

    fireEvent.click(await screen.findByText("choose folder…"));
    await screen.findByText("✕ not a directory");

    expect(container.querySelectorAll(".ink").length).toBe(1);
  });

  it("u2_when_the_daemon_exits_the_centre_shows_the_exit_code_in_ink", async () => {
    const app = createFakeApp();
    app.opened.project = PROJECT;
    const { container } = renderApp(app);
    await waitFor(() => expect(headerOf(container)).toBe("roundup   payments-api"));

    app.exitDaemon({ code: 3 });

    expect(within(region("centre")).getByText("✕ daemon exited 3").className).toBe("ink");
  });

  it("u2_when_a_signal_ended_the_daemon_the_centre_says_by_signal", async () => {
    const app = createFakeApp();
    app.opened.project = PROJECT;
    const { container } = renderApp(app);
    await waitFor(() => expect(headerOf(container)).toBe("roundup   payments-api"));

    app.exitDaemon({ code: null });

    expect(within(region("centre")).getByText("✕ daemon exited by signal").className).toBe("ink");
  });

  it("u3_a_failed_rail_refetch_shows_the_message_in_ink_in_the_centre", async () => {
    const app = createFakeApp();
    app.opened.project = PROJECT;
    const { container } = renderApp(app);
    await waitFor(() => expect(headerOf(container)).toBe("roundup   payments-api"));
    app.handlers["rail.tree"] = () => Promise.reject(new RpcError(-32603, "daemon is gone"));

    app.emit({ actor: { kind: "user", id: "you", parent: null }, name: "rail.changed" });

    expect((await within(region("centre")).findByText("✕ daemon is gone")).className).toBe("ink");
  });
});
