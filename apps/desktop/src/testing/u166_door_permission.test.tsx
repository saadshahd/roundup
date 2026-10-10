// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { App } from "../App";
import type { AppSeam } from "../app/seam";
import { fakeEmulators } from "../terminal/paneHarness";
import { createServedApp } from "./realSeam";
import { startRealDaemon } from "./realDaemonFixture";
import type { StartedDaemon } from "./realDaemonFixture";

let real: StartedDaemon;

beforeEach(async () => {
  real = await startRealDaemon({
    FAKE_CLAUDE_ON_PROMPT_ASK: JSON.stringify({ tool_name: "Bash", tool_input: { command: "echo $prompt" } }),
    FAKE_CLAUDE_ON_PROMPT: JSON.stringify({ name: "todo_create", arguments: { title: "$prompt" } }),
  });
}, 120_000);

afterEach(async () => {
  cleanup();
  await real?.stop();
});

const mount = () => {
  const app: AppSeam = createServedApp(real.base);

  render(() => <App app={app} reducedMotion={() => false} clock={Date.now} createEmulator={fakeEmulators().factory} />);

  return app;
};

const settle = async (answer: "allow" | "deny", prompt: string) => {
  const app = mount();
  const rail = await screen.findByRole("region", { name: "rail" });
  const rooms = async () => (await app.rpc("rail.tree", null)).filter((node) => node.kind === "room");
  const ready = async () => expect((await app.rpc("rail.tree", null)).map((node) => node.status?.label)).not.toContain("starting");
  const messages = async () => (await app.rpc("message.list", { to: null, status: null })).map((message) => [message.body, message.status]);

  fireEvent.click(await screen.findByRole("button", { name: "start a Room" }));
  fireEvent.click(await within(rail).findByRole("button", { name: /start Door/ }));
  await waitFor(ready, { timeout: 30_000 });

  // `+ room` is disabled while the Door is pending (U143).
  const add = await within(rail).findByRole("button", { name: /^\s*room$/ });

  await waitFor(() => expect(add).toHaveProperty("disabled", false), { timeout: 30_000 });
  fireEvent.click(add);
  await waitFor(async () => expect(await rooms()).toHaveLength(2), { timeout: 30_000 });
  fireEvent.click(await within(rail).findByRole("button", { name: /start Door/ }));
  await waitFor(ready, { timeout: 30_000 });

  const [first, second] = await rooms();

  fireEvent.click(rail.querySelector(`[data-id="${first?.id}"] .name`) ?? rail);
  fireEvent.input(await screen.findByRole("textbox", { name: "message" }), { target: { value: prompt } });
  fireEvent.click(await screen.findByRole("button", { name: "send" }));

  await waitFor(async () => expect(await messages()).toContainEqual([prompt, "delivered"]), { timeout: 30_000 });

  const card = await screen.findByRole("region", { name: /Decision for/ }, { timeout: 30_000 });

  await waitFor(async () => expect((await rooms()).find((room) => room.id === first?.id)?.status?.kind).toBe("needs-you"), { timeout: 30_000 });

  const otherBefore = (await rooms()).find((room) => room.id === second?.id)?.status?.kind;

  expect(otherBefore).not.toBe("needs-you");

  fireEvent.click(within(card).getByRole("button", { name: answer }));

  await waitFor(() => expect(screen.queryByRole("region", { name: /Decision for/ })).toBeNull(), { timeout: 30_000 });
  await waitFor(async () => expect((await rooms()).find((room) => room.id === first?.id)?.status?.kind).toBe("idle"), { timeout: 30_000 });
  expect(await messages()).toContainEqual([prompt, "delivered"]);
  expect((await rooms()).find((room) => room.id === second?.id)?.status?.kind).toBe(otherBefore);

  if (answer === "allow") await waitFor(async () => expect((await app.rpc("todo.list", null)).map((todo) => todo.title)).toContain(prompt), { timeout: 30_000 });

  cleanup();

  const reopened = mount();

  await waitFor(async () => expect((await reopened.rpc("rail.tree", null)).filter((node) => node.kind === "room").map((room) => room.status?.kind)).not.toContain("needs-you"), { timeout: 30_000 });
  expect(await messages()).toContainEqual([prompt, "delivered"]);

  return (await reopened.rpc("todo.list", null)).map((todo) => todo.title);
};

describe("u166 a Door's answered permission ends with the Room idle on the real Daemon", () => {
  it("u166_allow_runs_the_tool_call_and_the_room_is_idle_with_its_message_delivered", async () => {
    expect(await settle("allow", "ship a")).toContain("ship a");
  }, 240_000);

  it("u166_deny_makes_no_tool_call_and_the_room_is_idle_with_its_message_delivered", async () => {
    expect(await settle("deny", "ship b")).not.toContain("ship b");
  }, 240_000);
});
