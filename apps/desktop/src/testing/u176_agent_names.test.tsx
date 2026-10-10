// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, beforeEach, expect, it } from "vitest";
import { App } from "../App";
import type { AppSeam } from "../app/seam";
import { fakeEmulators } from "../terminal/paneHarness";
import { createServedApp } from "./realSeam";
import { startRealDaemon } from "./realDaemonFixture";
import type { StartedDaemon } from "./realDaemonFixture";

let real: StartedDaemon;

beforeEach(async () => {
  real = await startRealDaemon({ FAKE_CLAUDE_ON_PROMPT: JSON.stringify({ name: "todo_create", arguments: { title: "$prompt" } }) });
}, 120_000);

afterEach(async () => {
  cleanup();
  await real?.stop();
});

it("u176_two_agents_added_to_one_workstream_are_told_apart_in_the_rail", async () => {
  const app: AppSeam = createServedApp(real.base);

  render(() => <App app={app} reducedMotion={() => false} clock={Date.now} createEmulator={fakeEmulators().factory} />);

  const rail = await screen.findByRole("region", { name: "rail" });
  const agents = async () => (await app.rpc("rail.tree", null)).filter((node) => node.kind === "agent");

  fireEvent.click((await screen.findAllByRole("button", { name: "new Workstream" }))[0]!);

  for (const count of [1, 2]) {
    const add = await within(rail).findByText("agent", { selector: ".rail-adds button" }, { timeout: 30_000 });

    await waitFor(() => expect(add).toHaveProperty("disabled", false), { timeout: 30_000 });
    fireEvent.click(add);
    await waitFor(async () => expect(await agents()).toHaveLength(count), { timeout: 30_000 });
    await waitFor(async () => expect((await agents())[count - 1]!.status?.label).not.toBe("starting"), { timeout: 30_000 });
    await waitFor(async () => expect(await app.rpc("agent.prompt", { id: (await agents())[count - 1]!.id, text: "what should this do" })).toBeNull(), { timeout: 30_000 });
    await waitFor(async () => expect((await agents())[count - 1]!.name).not.toBe("new-agent"), { timeout: 60_000 });
  }

  const names = (await agents()).map((node) => node.name);

  expect(new Set(names).size).toBe(2);
  await waitFor(() => {
    for (const name of names) expect(within(rail).getAllByText(name).length).toBeGreaterThan(0);
  });
}, 180_000);
