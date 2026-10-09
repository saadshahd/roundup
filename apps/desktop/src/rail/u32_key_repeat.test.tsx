import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { agent, room } from "../testing/nodes";
import { mountRail, railCallsTo } from "./railFixture";

afterEach(cleanup);

const REPEATS = 20;

const press = (key: string, held: KeyboardEventInit) =>
  fireEvent.keyDown(document, { key, ...held });

const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

/** The key goes down, then repeats at the keyboard's rate: the previous spawn call has answered before each repeat. */
const hold = async (key: string, held: KeyboardEventInit) => {
  press(key, held);

  for (let i = 0; i < REPEATS; i++) {
    await tick();
    press(key, { ...held, repeat: true });
  }

  await tick();
};

describe("u32 a held chord spawns once", () => {
  it("u32_holding_cmd_n_spawns_one_agent_not_one_per_key_repeat", async () => {
    const mounted = await mountRail([room("g")]);
    mounted.app.handlers["agent.spawn"] = () =>
      agent("fresh", "idle", "starting");

    await hold("n", { metaKey: true });

    expect(railCallsTo(mounted.app, "agent.spawn").length).toBe(1);
  });

  it("u32_holding_cmd_t_spawns_one_terminal_not_one_per_key_repeat", async () => {
    const mounted = await mountRail([room("g")]);
    mounted.app.handlers["rail.spawnTerminal"] = () =>
      agent("fresh", "idle", "starting");

    await hold("t", { metaKey: true });

    expect(railCallsTo(mounted.app, "rail.spawnTerminal").length).toBe(1);
  });

  it("u32_a_repeat_of_shift_cmd_n_alone_opens_no_prompt_field", async () => {
    await mountRail([room("g")]);

    press("n", { metaKey: true, shiftKey: true, repeat: true });
    await Promise.resolve();

    expect(screen.queryByLabelText("prompt")).toBeNull();
  });

  it("u32_a_repeat_is_still_handled_by_the_webview_so_the_browser_never_sees_it", async () => {
    await mountRail([room("g")]);

    const handled = [
      press("n", { metaKey: true, repeat: true }),
      press("t", { metaKey: true, repeat: true }),
      press("n", { metaKey: true, shiftKey: true, repeat: true }),
    ];

    expect(handled).toEqual([false, false, false]);
  });

  it("u32_a_new_press_after_the_repeats_spawns_again", async () => {
    const mounted = await mountRail([room("g")]);
    mounted.app.handlers["agent.spawn"] = () =>
      agent("fresh", "idle", "starting");

    await hold("n", { metaKey: true });
    expect(railCallsTo(mounted.app, "agent.spawn").length).toBe(1);
    press("n", { metaKey: true });

    await waitFor(() =>
      expect(railCallsTo(mounted.app, "agent.spawn").length).toBe(2),
    );
  });
});
