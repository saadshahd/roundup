import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import { afterEach, describe, expect, it } from "vitest";
import { agent, group } from "../testing/nodes";
import styles from "./styles.css?inline";
import { callsTo, mountRail } from "./railFixture";

afterEach(cleanup);

const SPAWNED = agent("fresh", "idle", "starting");

const chord = (key: string) => fireEvent.keyDown(document, { key, metaKey: true });

describe("u32 spawn shortcuts and pinned actions", () => {
  it("u32_cmd_n_spawns_an_agent_under_the_selected_group", async () => {
    const mounted = await mountRail([group("g")]);
    mounted.app.handlers["agent.spawn"] = () => SPAWNED;
    mounted.rail.select("g");

    chord("n");

    await waitFor(() => expect(callsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: null, parent: "g" }]));
  });

  it("u32_cmd_t_spawns_a_terminal_under_the_selected_group", async () => {
    const mounted = await mountRail([group("g")]);
    mounted.app.handlers["rail.spawnTerminal"] = () => SPAWNED;
    mounted.rail.select("g");

    chord("t");

    await waitFor(() => expect(callsTo(mounted.app, "rail.spawnTerminal")).toEqual([{ cwd: "/p", parent: "g" }]));
  });

  it("u32_a_second_chord_while_a_spawn_is_in_flight_calls_nothing", async () => {
    const mounted = await mountRail([group("g")]);
    mounted.app.handlers["agent.spawn"] = () => new Promise(() => {});

    chord("n");
    chord("n");
    await Promise.resolve();

    expect(callsTo(mounted.app, "agent.spawn").length).toBe(1);
  });

  it("u32_the_chords_do_nothing_after_the_daemon_exited", async () => {
    const [exit] = createSignal({ code: 1 });
    const mounted = await mountRail([group("g")], [], exit);

    chord("n");
    chord("t");
    await Promise.resolve();

    expect([callsTo(mounted.app, "agent.spawn"), callsTo(mounted.app, "rail.spawnTerminal")]).toEqual([[], []]);
  });

  it("u32_a_failed_chord_spawn_shows_the_error_line", async () => {
    const mounted = await mountRail([group("g")]);
    mounted.app.handlers["agent.spawn"] = () => {
      throw new Error("no room");
    };

    chord("n");

    expect((await screen.findByText(/no room/)).textContent).toBe("✕ no room");
  });

  it("u32_the_action_line_is_pinned_to_the_bottom_edge_of_the_rail", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;
    await mountRail([group("g")]);
    const line = getComputedStyle(screen.getByText("+ agent").parentElement as Element);
    const pinned = [line.position, line.bottom];

    sheet.remove();

    expect(pinned).toEqual(["sticky", "0px"]);
  });
});
