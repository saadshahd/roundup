import { cleanup, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { createFakeApp } from "../testing/fakeApp";

const openApp = async () => {
  const app = createFakeApp();

  app.opened.project = { name: "payments-api", path: "/p" };
  app.handlers["rail.tree"] = () => [];
  app.handlers["terminal.list"] = () => [];

  const { container } = render(() => <App app={app} reducedMotion={() => false} clock={() => 0} />);

  await waitFor(() => expect(container.querySelector(".pane")).not.toBeNull());

  return { app, header: screen.getByRole("banner") };
};

afterEach(cleanup);

describe("u25 the Daemon is gone", () => {
  it("u25_the_header_names_the_exit_code_in_ink_after_the_project", async () => {
    const { app, header } = await openApp();

    app.exitDaemon({ code: 1 });

    expect(header.textContent).toBe("roundup   payments-api   daemon exited 1   reopen");
    expect(within(header).getByText("daemon exited 1").className).toBe("ink");
  });

  it("u25_a_signal_exit_reads_by_signal", async () => {
    const { app, header } = await openApp();

    app.exitDaemon({ code: null });

    expect(within(header).getByText("daemon exited by signal").className).toBe("ink");
  });

  it("u25_the_exit_shows_in_the_header_only", async () => {
    const { app } = await openApp();

    app.exitDaemon({ code: 1 });

    expect(screen.getAllByText("daemon exited 1")).toHaveLength(1);
  });

  it("u25_the_spawn_actions_are_disabled", async () => {
    const { app } = await openApp();

    app.exitDaemon({ code: 1 });

    for (const button of screen.getAllByRole("button", { name: "new Workstream" })) {
      expect(button.hasAttribute("aria-disabled")).toBe(true);
    }
  });
});
