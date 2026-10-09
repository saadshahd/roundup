// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { App } from "../App";
import type { AppSeam } from "../app/seam";
import { fakeEmulators } from "../terminal/paneHarness";
import { createServedApp } from "./realSeam";
import { startRealDaemon } from "./realDaemonFixture";
import type { StartedDaemon } from "./realDaemonFixture";

let real: StartedDaemon;

beforeAll(async () => {
  real = await startRealDaemon({
    FAKE_CLAUDE_ON_PROMPT: JSON.stringify({ name: "todo_create", arguments: { title: "$prompt" } }),
  });
}, 120_000);

afterAll(async () => {
  await real?.stop();
});

afterEach(cleanup);

const mount = () => {
  const app: AppSeam = createServedApp(real.base);

  render(() => <App app={app} reducedMotion={() => false} clock={Date.now} createEmulator={fakeEmulators().factory} />);

  return app;
};

describe("u146 a Door answers a Thread message in the real-Daemon harness", () => {
  it("u146_a_message_to_a_started_door_is_delivered_and_its_todo_shows", async () => {
    const app = mount();

    fireEvent.click(await screen.findByRole("button", { name: "start a Room" }));

    const rail = await screen.findByRole("region", { name: "rail" });

    fireEvent.click(await within(rail).findByRole("button", { name: /start Door/ }));

    const input = await screen.findByRole("textbox", { name: "message" });

    // The Door takes its first Message once SessionStart has readied it (H12).
    await waitFor(async () => expect((await app.rpc("rail.tree", null)).map((node) => node.status?.label)).not.toContain("starting"), { timeout: 30_000 });
    fireEvent.input(input, { target: { value: "ship j3" } });
    fireEvent.click(await screen.findByRole("button", { name: "send" }));

    await waitFor(async () => expect((await app.rpc("message.list", { to: null, status: null })).map((message) => [message.body, message.status])).toContainEqual(["ship j3", "delivered"]), { timeout: 30_000 });
    await waitFor(async () => expect((await app.rpc("todo.list", null)).map((todo) => todo.title)).toContain("ship j3"), { timeout: 30_000 });

    const thread = screen.getByRole("log", { name: "thread" });

    await waitFor(() => expect(thread.querySelector("[data-message][data-status]")?.getAttribute("data-status")).toBe("delivered"));
    await waitFor(async () => expect((await app.rpc("rail.tree", null)).map((node) => node.status?.label)).not.toContain("starting"));

    cleanup();
    mount();

    const reopened = await screen.findByRole("log", { name: "thread" });

    await waitFor(() => expect(reopened.textContent).toContain("ship j3"));
    expect(await screen.findByText(/ship j3/, { selector: "[data-todo] *" })).toBeTruthy();
  }, 120_000);
});
