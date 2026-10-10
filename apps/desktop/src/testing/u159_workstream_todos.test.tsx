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

describe("u159 a Workstream's Todos are found from the Workstream on the real Daemon", () => {
  it("u159_the_todo_a_door_made_is_listed_by_this_workstream_and_another_workstream_has_none", async () => {
    const app: AppSeam = createServedApp(real.base);

    render(() => <App app={app} reducedMotion={() => false} clock={Date.now} createEmulator={fakeEmulators().factory} />);

    fireEvent.click((await screen.findAllByRole("button", { name: "new Workstream" }))[0]!);

    const rail = await screen.findByRole("region", { name: "rail" });

    fireEvent.click(await within(rail).findByRole("button", { name: /start Door/ }));

    const input = await screen.findByRole("textbox", { name: "message" });

    await waitFor(async () => expect((await app.rpc("rail.tree", null)).map((node) => node.status?.label)).not.toContain("starting"), { timeout: 30_000 });
    fireEvent.input(input, { target: { value: "ship j4" } });
    fireEvent.click(await screen.findByRole("button", { name: "send" }));

    const workstream = (await app.rpc("rail.tree", null)).find((node) => node.kind === "workstream");

    await waitFor(async () => expect((await app.rpc("todo.list", null)).find((todo) => todo.title === "ship j4")?.home).toBe(workstream?.id), { timeout: 30_000 });

    const shelf = await screen.findByRole("region", { name: "todos" });

    fireEvent.click(await within(shelf).findByRole("radio", { name: "this workstream" }));
    await waitFor(() => expect(shelf.querySelector("[data-shelf-row]")?.textContent).toContain("ship j4"));

    fireEvent.click(await within(rail).findByRole("button", { name: /^\s*new Workstream$/ }));
    await waitFor(async () => expect((await app.rpc("rail.tree", null)).filter((node) => node.kind === "workstream")).toHaveLength(2));
    await waitFor(() => expect(within(shelf).getByRole("radio", { name: "all" })).toHaveProperty("checked", true));
    fireEvent.click(within(shelf).getByRole("radio", { name: "this workstream" }));

    expect(await within(shelf).findByText("no todos in this workstream")).toBeTruthy();
  }, 120_000);
});
