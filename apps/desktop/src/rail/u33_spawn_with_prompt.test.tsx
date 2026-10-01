import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import { afterEach, describe, expect, it } from "vitest";
import { agent, event, group } from "../testing/nodes";
import { callsTo, mountRail } from "./railFixture";

afterEach(cleanup);

const SPAWNED = agent("fresh", "idle", "starting");

const chord = (key: string, held: KeyboardEventInit = { metaKey: true, shiftKey: true }) =>
  fireEvent.keyDown(document, { key, ...held });

const field = () => screen.getByLabelText<HTMLInputElement>("prompt");

describe("u33 spawn with a prompt", () => {
  it("u33_shift_cmd_n_opens_a_focused_prompt_field_at_the_top_of_the_rail", async () => {
    await mountRail([group("g")]);

    chord("n");
    await Promise.resolve();

    expect(document.activeElement).toBe(field());
  });

  it("u33_enter_spawns_with_the_typed_prompt_and_the_selected_groups_parent", async () => {
    const mounted = await mountRail([group("g")]);
    mounted.app.handlers["agent.spawn"] = () => SPAWNED;
    mounted.rail.select("g");

    chord("n");
    fireEvent.input(field(), { target: { value: "fix the thing" } });
    fireEvent.keyDown(field(), { key: "Enter" });

    await waitFor(() =>
      expect(callsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: "fix the thing", parent: "g" }]),
    );
  });

  it("u33_enter_on_an_empty_field_spawns_with_no_prompt", async () => {
    const mounted = await mountRail([group("g")]);
    mounted.app.handlers["agent.spawn"] = () => SPAWNED;

    chord("n");
    fireEvent.keyDown(field(), { key: "Enter" });

    await waitFor(() =>
      expect(callsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: null, parent: null }]),
    );
  });

  it("u33_enter_on_a_field_of_only_spaces_spawns_with_no_prompt", async () => {
    const mounted = await mountRail([group("g")]);
    mounted.app.handlers["agent.spawn"] = () => SPAWNED;

    chord("n");
    fireEvent.input(field(), { target: { value: "   " } });
    fireEvent.keyDown(field(), { key: "Enter" });

    await waitFor(() =>
      expect(callsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: null, parent: null }]),
    );
  });

  it("u33_the_new_row_becomes_selected_once_the_tree_has_it", async () => {
    const tree = [group("g")];
    const mounted = await mountRail(tree);
    mounted.app.handlers["agent.spawn"] = () => {
      tree.push(SPAWNED);
      mounted.app.emit(event({ name: "rail.changed" }));

      return SPAWNED;
    };

    chord("n");
    fireEvent.keyDown(field(), { key: "Enter" });

    await waitFor(() => expect(mounted.rail.selected()).toBe("fresh"));
    expect(screen.queryByLabelText("prompt")).toBeNull();
  });

  it("u33_escape_closes_the_field_and_calls_nothing", async () => {
    const mounted = await mountRail([group("g")]);

    chord("n");
    fireEvent.keyDown(field(), { key: "Escape" });

    expect([screen.queryByLabelText("prompt"), callsTo(mounted.app, "agent.spawn")]).toEqual([null, []]);
  });

  it("u33_a_failed_call_shows_the_error_line_and_keeps_the_fields_text", async () => {
    const mounted = await mountRail([group("g")]);
    mounted.app.handlers["agent.spawn"] = () => {
      throw new Error("no room");
    };

    chord("n");
    fireEvent.input(field(), { target: { value: "fix the thing" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    const shown = await screen.findByText("✕ no room");

    expect([shown.textContent, field().value]).toEqual(["✕ no room", "fix the thing"]);
  });

  it("u33_shift_cmd_t_still_does_nothing", async () => {
    const mounted = await mountRail([group("g")]);

    chord("t");
    await Promise.resolve();

    expect([screen.queryByLabelText("prompt"), callsTo(mounted.app, "rail.spawnTerminal")]).toEqual([null, []]);
  });

  it("u33_cmd_n_still_spawns_with_no_prompt_and_no_field", async () => {
    const mounted = await mountRail([group("g")]);
    mounted.app.handlers["agent.spawn"] = () => SPAWNED;

    chord("n", { metaKey: true });

    await waitFor(() =>
      expect(callsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: null, parent: null }]),
    );
    expect(screen.queryByLabelText("prompt")).toBeNull();
  });

  it("u33_with_caps_lock_on_the_chord_still_opens_the_field", async () => {
    await mountRail([group("g")]);

    chord("N");

    expect(screen.queryByLabelText("prompt")).not.toBeNull();
  });

  it.each<[string, KeyboardEventInit]>([
    ["cmd shift and ctrl", { metaKey: true, shiftKey: true, ctrlKey: true }],
    ["cmd shift and alt", { metaKey: true, shiftKey: true, altKey: true }],
    ["shift alone", { shiftKey: true }],
  ])("u33_shift_cmd_n_does_nothing_with_%s", async (_, held) => {
    await mountRail([group("g")]);

    chord("n", held);

    expect(screen.queryByLabelText("prompt")).toBeNull();
  });

  it("u33_shift_cmd_n_does_nothing_while_a_spawn_is_in_flight", async () => {
    const mounted = await mountRail([group("g")]);
    mounted.app.handlers["agent.spawn"] = () => new Promise(() => {});

    chord("n", { metaKey: true });
    chord("n");
    await Promise.resolve();

    expect(screen.queryByLabelText("prompt")).toBeNull();
  });

  it("u33_shift_cmd_n_does_nothing_after_the_daemon_exited", async () => {
    const [exit] = createSignal({ code: 1 });
    await mountRail([group("g")], [], exit);

    chord("n");

    expect(screen.queryByLabelText("prompt")).toBeNull();
  });

  it("u33_a_second_enter_while_the_spawn_is_in_flight_calls_nothing_more", async () => {
    const mounted = await mountRail([group("g")]);
    mounted.app.handlers["agent.spawn"] = () => new Promise(() => {});

    chord("n");
    fireEvent.keyDown(field(), { key: "Enter" });
    fireEvent.keyDown(field(), { key: "Enter" });
    await Promise.resolve();

    expect(callsTo(mounted.app, "agent.spawn").length).toBe(1);
  });
});
