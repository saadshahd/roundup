import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import { afterEach, describe, expect, it } from "vitest";
import { mountRailAndPane } from "../terminal/paneHarness";
import { agent, event, workstream } from "../testing/nodes";
import { mountRail, pinnedAdd, railCallsTo } from "./railFixture";

afterEach(cleanup);

const SPAWNED = agent("fresh", "idle", "starting");

const chord = (key: string, held: KeyboardEventInit = { metaKey: true, shiftKey: true }) =>
  fireEvent.keyDown(document, { key, ...held });

const field = () => screen.getByLabelText<HTMLInputElement>("prompt");

describe("u33 spawn with a prompt", () => {
  it("u33_shift_cmd_n_opens_a_focused_prompt_field_at_the_top_of_the_rail", async () => {
    await mountRail([workstream("g")]);

    chord("n");
    // SpawnPromptField's ref focuses the input from a queued microtask (SpawnPromptField.tsx); this flushes it.
    await Promise.resolve();

    expect(document.activeElement).toBe(field());
  });

  it("u33_shift_cmd_n_is_handled_by_the_webview_so_the_browser_never_sees_it", async () => {
    await mountRail([workstream("g")]);

    const handled = fireEvent.keyDown(document, { key: "n", metaKey: true, shiftKey: true });

    expect(handled).toBe(false);
  });

  it("u33_the_prompt_field_sits_before_the_tree", async () => {
    await mountRail([workstream("g")]);

    chord("n");

    const position = field().compareDocumentPosition(screen.getByRole("tree"));

    expect(position & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("u33_typing_an_ordinary_key_goes_into_the_field_and_never_closes_it", async () => {
    const mounted = await mountRail([workstream("g")]);

    chord("n");
    fireEvent.input(field(), { target: { value: "f" } });
    fireEvent.keyDown(field(), { key: "f" });

    expect(screen.queryByLabelText("prompt")).toBe(field());
    expect(railCallsTo(mounted.app, "agent.spawn")).toEqual([]);
  });

  it("u33_enter_trims_leading_and_trailing_whitespace_from_the_prompt", async () => {
    const mounted = await mountRail([workstream("g")]);
    mounted.app.handlers["agent.spawn"] = () => SPAWNED;

    chord("n");
    fireEvent.input(field(), { target: { value: " fix " } });
    fireEvent.keyDown(field(), { key: "Enter" });

    await waitFor(() =>
      expect(railCallsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: "fix", parent: null }]),
    );
  });

  it("u33_enter_spawns_with_the_typed_prompt_and_the_selected_groups_parent", async () => {
    const mounted = await mountRail([workstream("g")]);
    mounted.app.handlers["agent.spawn"] = () => SPAWNED;
    mounted.rail.select("g");

    chord("n");
    fireEvent.input(field(), { target: { value: "fix the thing" } });
    fireEvent.keyDown(field(), { key: "Enter" });

    await waitFor(() =>
      expect(railCallsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: "fix the thing", parent: "g" }]),
    );
  });

  it("u33_enter_on_an_empty_field_spawns_with_no_prompt", async () => {
    const mounted = await mountRail([workstream("g")]);
    mounted.app.handlers["agent.spawn"] = () => SPAWNED;

    chord("n");
    fireEvent.keyDown(field(), { key: "Enter" });

    await waitFor(() =>
      expect(railCallsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: null, parent: null }]),
    );
  });

  it("u33_enter_on_a_field_of_only_spaces_spawns_with_no_prompt", async () => {
    const mounted = await mountRail([workstream("g")]);
    mounted.app.handlers["agent.spawn"] = () => SPAWNED;

    chord("n");
    fireEvent.input(field(), { target: { value: "   " } });
    fireEvent.keyDown(field(), { key: "Enter" });

    await waitFor(() =>
      expect(railCallsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: null, parent: null }]),
    );
  });

  it("u33_the_new_row_becomes_selected_once_the_tree_has_it", async () => {
    const tree = [workstream("g")];
    const mounted = await mountRail(tree);
    mounted.app.handlers["agent.spawn"] = () => {
      tree.push(SPAWNED);
      mounted.app.emit(event({ name: "rail.changed" }));

      return SPAWNED;
    };

    chord("n");
    fireEvent.keyDown(field(), { key: "Enter" });

    await waitFor(() => expect(mounted.rail.selected()).toBe("fresh"));
  });

  it("u33_a_successful_spawn_closes_the_field", async () => {
    const mounted = await mountRail([workstream("g")]);
    mounted.app.handlers["agent.spawn"] = () => SPAWNED;

    chord("n");
    fireEvent.keyDown(field(), { key: "Enter" });

    await waitFor(() => expect(screen.queryByLabelText("prompt")).toBeNull());
  });

  it("u33_escape_closes_the_field_and_calls_nothing", async () => {
    const mounted = await mountRail([workstream("g")]);

    chord("n");
    fireEvent.keyDown(field(), { key: "Escape" });

    expect([screen.queryByLabelText("prompt"), railCallsTo(mounted.app, "agent.spawn")]).toEqual([null, []]);
  });

  it("u33_a_failed_call_shows_the_error_line_and_keeps_the_fields_text", async () => {
    const mounted = await mountRail([workstream("g")]);
    mounted.app.handlers["agent.spawn"] = () => {
      throw new Error("no workstream");
    };

    chord("n");
    fireEvent.input(field(), { target: { value: "fix the thing" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    const shown = await screen.findByText("no workstream");

    expect([shown.textContent, field().value]).toEqual(["no workstream", "fix the thing"]);
  });

  it("u33_cmd_n_still_spawns_with_no_prompt_and_no_field", async () => {
    const mounted = await mountRail([workstream("g")]);
    mounted.app.handlers["agent.spawn"] = () => SPAWNED;

    chord("n", { metaKey: true });

    await waitFor(() =>
      expect(railCallsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: null, parent: null }]),
    );
    expect(screen.queryByLabelText("prompt")).toBeNull();
  });

  it("u33_with_caps_lock_on_the_chord_still_opens_the_field", async () => {
    await mountRail([workstream("g")]);

    chord("N");

    expect(screen.queryByLabelText("prompt")).not.toBeNull();
  });

  it.each<[string, KeyboardEventInit]>([
    ["cmd shift and ctrl", { metaKey: true, shiftKey: true, ctrlKey: true }],
    ["cmd shift and alt", { metaKey: true, shiftKey: true, altKey: true }],
    ["shift alone", { shiftKey: true }],
  ])("u33_shift_cmd_n_does_nothing_with_%s", async (_, held) => {
    await mountRail([workstream("g")]);

    chord("n", held);

    expect(screen.queryByLabelText("prompt")).toBeNull();
  });

  it("u33_shift_cmd_n_does_nothing_while_a_spawn_is_in_flight", async () => {
    const mounted = await mountRail([workstream("g")]);
    mounted.app.handlers["agent.spawn"] = () => new Promise(() => {});

    chord("n", { metaKey: true });
    chord("n");
    // guarded() sets pending() synchronously, so canSpawn() already reads false; this just gives a tick before asserting nothing slipped through.
    await Promise.resolve();

    expect(screen.queryByLabelText("prompt")).toBeNull();
  });

  it("u33_shift_cmd_n_does_nothing_after_the_daemon_exited", async () => {
    const [exit] = createSignal({ code: 1 });
    await mountRail([workstream("g")], [], exit);

    chord("n");

    expect(screen.queryByLabelText("prompt")).toBeNull();
  });

  it("u33_enter_after_the_daemon_exited_calls_nothing", async () => {
    const [exit, setExit] = createSignal<{ code: number } | null>(null);
    const mounted = await mountRail([workstream("g")], [], exit);
    mounted.app.handlers["agent.spawn"] = () => SPAWNED;

    chord("n");
    setExit({ code: 1 });
    fireEvent.keyDown(field(), { key: "Enter" });

    expect(railCallsTo(mounted.app, "agent.spawn")).toEqual([]);
  });

  it("u33_a_second_enter_while_the_spawn_is_in_flight_calls_nothing_more", async () => {
    const mounted = await mountRail([workstream("g")]);
    mounted.app.handlers["agent.spawn"] = () => new Promise(() => {});

    chord("n");
    fireEvent.keyDown(field(), { key: "Enter" });
    fireEvent.keyDown(field(), { key: "Enter" });
    // The first Enter sets pending() synchronously, so the second reads canSpawn() false; this just gives a tick before asserting no second call arrived.
    await Promise.resolve();

    expect(railCallsTo(mounted.app, "agent.spawn").length).toBe(1);
  });

  it("u33_cmd_n_does_nothing_while_the_prompt_field_is_already_open", async () => {
    const mounted = await mountRail([workstream("g")]);

    chord("n");
    fireEvent.input(field(), { target: { value: "keep me" } });
    chord("n", { metaKey: true });

    expect(railCallsTo(mounted.app, "agent.spawn")).toEqual([]);
    expect(field().value).toBe("keep me");
    expect(screen.queryByLabelText("prompt")).not.toBeNull();
  });

  it("u33_shift_cmd_n_does_nothing_while_the_field_is_already_open", async () => {
    const mounted = await mountRail([workstream("g")]);

    chord("n");
    fireEvent.input(field(), { target: { value: "keep me" } });
    chord("n");

    expect(railCallsTo(mounted.app, "agent.spawn")).toEqual([]);
    expect(field().value).toBe("keep me");
    expect(screen.queryByLabelText("prompt")).not.toBeNull();
  });

  it("u33_cmd_t_still_spawns_a_terminal_while_the_field_is_open_and_leaves_it_open", async () => {
    const mounted = await mountRail([workstream("g")]);
    mounted.app.handlers["rail.spawnTerminal"] = () => SPAWNED;

    chord("n");
    fireEvent.input(field(), { target: { value: "keep me" } });
    chord("t", { metaKey: true });

    await waitFor(() =>
      expect(railCallsTo(mounted.app, "rail.spawnTerminal")).toEqual([{ cwd: "/p", parent: null }]),
    );
    expect(field().value).toBe("keep me");
  });

  it.each(["n", "t"])(
    "u33_cmd_%s_is_a_bound_shortcut_so_it_never_reaches_the_field_while_the_field_is_open",
    async (key) => {
      const mounted = await mountRail([workstream("g")]);
      mounted.app.handlers["rail.spawnTerminal"] = () => SPAWNED;

      chord("n");
      fireEvent.input(field(), { target: { value: "keep me" } });
      const handled = chord(key, { metaKey: true });

      expect(handled).toBe(false);
    },
  );

  it.each(["v", "a", "z"])(
    "u33_cmd_%s_is_not_a_bound_shortcut_so_it_is_typed_into_the_field_and_never_closes_it",
    async (key) => {
      await mountRail([workstream("g")]);

      chord("n");
      fireEvent.input(field(), { target: { value: "keep me" } });
      const handled = chord(key, { metaKey: true });

      expect(handled).toBe(true);
      expect(field().value).toBe("keep me");
      expect(screen.queryByLabelText("prompt")).not.toBeNull();
    },
  );

  it("u33_plus_agent_moves_focus_to_the_field_and_calls_nothing", async () => {
    const mounted = await mountRail([workstream("g")]);
    mounted.rail.select("g");

    chord("n");
    fireEvent.input(field(), { target: { value: "keep me" } });
    pinnedAdd().focus();

    fireEvent.click(screen.getByText("agent"));

    expect(document.activeElement).toBe(field());
    expect(railCallsTo(mounted.app, "agent.spawn")).toEqual([]);
    expect(field().value).toBe("keep me");
  });

  it("u33_plus_terminal_spawns_and_leaves_the_field_open_with_its_text", async () => {
    const mounted = await mountRail([workstream("g")]);
    mounted.app.handlers["rail.spawnTerminal"] = () => SPAWNED;
    mounted.rail.select("g");

    chord("n");
    fireEvent.input(field(), { target: { value: "keep me" } });
    fireEvent.click(screen.getByText("terminal"));

    await waitFor(() =>
      expect(railCallsTo(mounted.app, "rail.spawnTerminal")).toEqual([{ cwd: "/p", parent: "g" }]),
    );
    expect(field().value).toBe("keep me");
  });

  it("u33_plus_group_creates_a_group_and_leaves_the_field_open_with_its_text", async () => {
    const mounted = await mountRail([workstream("g")]);
    mounted.app.handlers["rail.createWorkstream"] = () => workstream("fresh");

    chord("n");
    fireEvent.input(field(), { target: { value: "keep me" } });
    fireEvent.click(pinnedAdd());

    await waitFor(() =>
      expect(railCallsTo(mounted.app, "rail.createWorkstream")).toEqual([{ name: "workstream", parent: null }]),
    );
    expect(field().value).toBe("keep me");
  });

  it("u33_the_new_row_takes_focus_into_the_pane", async () => {
    const tree = [workstream("g")];
    const mounted = await mountRailAndPane(tree);

    mounted.app.handlers["agent.spawn"] = () => {
      tree.push(SPAWNED);
      mounted.app.emit(event({ name: "rail.changed" }));

      return SPAWNED;
    };

    chord("n");
    fireEvent.keyDown(field(), { key: "Enter" });

    await waitFor(() => expect(mounted.connected.rail.selected()).toBe("fresh"));

    const screenEl = mounted.container.querySelector(".pane-screen");

    expect(screenEl?.contains(document.activeElement)).toBe(true);
    expect(screenEl?.querySelector('[data-terminal="t-fresh"]')).toBe(document.activeElement);
  });
});
