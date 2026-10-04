import { cleanup, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { createFakeApp } from "../testing/fakeApp";

const openApp = async () => {
  const app = createFakeApp();

  app.opened.project = { name: "payments-api", path: "/p" };
  app.handlers["rail.tree"] = () => [];
  app.handlers["terminal.list"] = () => [];
  render(() => <App app={app} reducedMotion={() => false} clock={() => 0} />);
  await waitFor(() => expect(screen.getByRole("region", { name: "centre" }).querySelector(".pane")).not.toBeNull());

  return app;
};

afterEach(cleanup);

describe("u13 the centre's error lines never move the screen", () => {
  it("u13_a_failed_rail_refetch_line_sits_in_the_panes_fixed_slot", async () => {
    const app = await openApp();

    app.handlers["rail.tree"] = () => {
      throw new Error("daemon is gone");
    };

    app.emit({ actor: { kind: "user", id: "you", parent: null }, name: "rail.changed" });

    expect((await screen.findByRole("alert", { name: "daemon is gone" })).closest(".pane-failure")).not.toBeNull();
    expect(screen.getByRole("alert").querySelector('svg.lucide-x[aria-hidden="true"]')).not.toBeNull();
  });
});
