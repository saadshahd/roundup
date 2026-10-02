import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import { afterEach, describe, expect, it } from "vitest";
import { agent, group } from "../testing/nodes";
import styles from "./styles.css?inline";
import { callsTo, mountRail } from "./railFixture";

afterEach(cleanup);

const SPAWNED = agent("fresh", "idle", "starting");

const chord = (key: string, held: KeyboardEventInit = { metaKey: true }) => fireEvent.keyDown(document, { key, ...held });

const spawnCalls = (mounted: Awaited<ReturnType<typeof mountRail>>) => [
  ...callsTo(mounted.app, "agent.spawn"),
  ...callsTo(mounted.app, "rail.spawnTerminal"),
];

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

  it.each<[string, KeyboardEventInit]>([
    ["no modifier", {}],
    ["ctrl", { ctrlKey: true }],
    ["alt", { altKey: true }],
    ["cmd and ctrl", { metaKey: true, ctrlKey: true }],
    ["cmd and alt", { metaKey: true, altKey: true }],
  ])("u32_a_plain_n_or_t_spawns_nothing_%s", async (_, held) => {
    const mounted = await mountRail([group("g")]);

    chord("n", held);
    chord("t", held);
    await Promise.resolve();

    expect(spawnCalls(mounted)).toEqual([]);
  });

  /** ⇧⌘N is U33's spawn-with-a-prompt chord; ⇧⌘T stays inert. */
  it("u32_shift_cmd_t_still_spawns_nothing", async () => {
    const mounted = await mountRail([group("g")]);

    chord("t", { metaKey: true, shiftKey: true });
    await Promise.resolve();

    expect(spawnCalls(mounted)).toEqual([]);
    expect(screen.queryByLabelText("prompt")).toBeNull();
  });

  it("u32_with_caps_lock_on_the_chord_still_spawns", async () => {
    const mounted = await mountRail([group("g")]);
    mounted.app.handlers["agent.spawn"] = () => SPAWNED;

    chord("N");

    await waitFor(() => expect(callsTo(mounted.app, "agent.spawn").length).toBe(1));
  });

  it("u32_the_chords_are_handled_by_the_webview_so_the_browser_never_sees_them", async () => {
    await mountRail([group("g")]);

    const handled = [
      fireEvent.keyDown(document, { key: "n", metaKey: true }),
      fireEvent.keyDown(document, { key: "t", metaKey: true }),
    ];

    const ignored = fireEvent.keyDown(document, { key: "n" });

    expect([handled, ignored]).toEqual([[false, false], true]);
  });

  it("u32_once_the_rail_is_gone_the_chords_call_nothing", async () => {
    const mounted = await mountRail([group("g")]);

    cleanup();
    chord("n");
    chord("t");
    await Promise.resolve();

    expect(spawnCalls(mounted)).toEqual([]);
  });

  it("u32_the_action_line_is_pinned_to_the_bottom_edge_of_the_rail", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;
    await mountRail([group("g")]);
    const line = getComputedStyle(screen.getByText("+ agent").closest(".rail-actions") ?? document.body);
    const pinned = [line.position, line.bottom, line.background];

    sheet.remove();

    expect(pinned).toEqual(["sticky", "0px", "var(--ground)"]);
  });
});
