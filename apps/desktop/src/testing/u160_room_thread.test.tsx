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
    FAKE_CLAUDE_ON_PROMPT: JSON.stringify({ name: "message_send", arguments: { to: "you", kind: "note", body: "done $prompt", replyTo: null } }),
  });
}, 120_000);

afterAll(async () => {
  await real?.stop();
});

afterEach(cleanup);

describe("u160 a Room's Thread holds only its own Messages on the real Daemon", () => {
  it("u160_each_rooms_thread_holds_its_message_and_its_doors_answer_and_not_the_others", async () => {
    const app: AppSeam = createServedApp(real.base);

    render(() => <App app={app} reducedMotion={() => false} clock={Date.now} createEmulator={fakeEmulators().factory} />);

    const rail = await screen.findByRole("region", { name: "rail" });
    const rooms = async () => (await app.rpc("rail.tree", null)).filter((node) => node.kind === "room");
    const ready = async () => expect((await app.rpc("rail.tree", null)).map((node) => node.status?.label)).not.toContain("starting");

    fireEvent.click(await screen.findByRole("button", { name: "start a Room" }));
    fireEvent.click(await within(rail).findByRole("button", { name: /start Door/ }));
    await waitFor(ready, { timeout: 30_000 });
    fireEvent.input(await screen.findByRole("textbox", { name: "message" }), { target: { value: "a" } });
    fireEvent.click(await screen.findByRole("button", { name: "send" }));
    await waitFor(async () => expect((await app.rpc("message.list", { to: null, status: null })).map((message) => message.body)).toContain("done a"), { timeout: 30_000 });

    fireEvent.click(await within(rail).findByRole("button", { name: /^\s*room$/ }));
    await waitFor(async () => expect(await rooms()).toHaveLength(2));
    fireEvent.click(await within(rail).findByRole("button", { name: /start Door/ }));
    await waitFor(ready, { timeout: 30_000 });
    fireEvent.input(await screen.findByRole("textbox", { name: "message" }), { target: { value: "b" } });
    fireEvent.click(await screen.findByRole("button", { name: "send" }));
    await waitFor(async () => expect((await app.rpc("message.list", { to: null, status: null })).map((message) => message.body)).toContain("done b"), { timeout: 30_000 });

    const bodies = async () => (await app.rpc("message.list", { to: null, status: null })).map((message) => message.body).toSorted();

    expect(await bodies()).toEqual(["a", "b", "done a", "done b"]);

    const thread = screen.getByRole("log", { name: "thread" });
    const lines = () => [...thread.querySelectorAll(".thread-line")].map((line) => line.textContent ?? "");

    await waitFor(() => expect(lines().join("\n")).toContain("done b"));

    expect(lines().join("\n")).toContain(" b");
    expect(lines().join("\n")).not.toMatch(/\b(done a|note a)\b/);

    const [first] = await rooms();

    fireEvent.click(rail.querySelector(`[data-id="${first?.id}"] .name`) ?? rail);
    await waitFor(() => expect(lines().join("\n")).toContain("done a"));

    expect(lines().join("\n")).not.toMatch(/\b(done b|note b)\b/);
  }, 180_000);
});
