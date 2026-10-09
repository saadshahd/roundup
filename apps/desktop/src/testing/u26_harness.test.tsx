import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { fakeEmulators } from "../terminal/paneHarness";
import { fromBase64 } from "../terminal/base64";
import { SEEDS, seedApp } from "./seeds";
import type { SeedName } from "./seeds";
import type { Event as DaemonEvent } from "@contracts/Event";
import { event } from "./nodes";

const NOW = 1_700_000_000_000;

const mount = (seed: SeedName) => {
  const controls = seedApp(seed, Date.now());
  render(() => <App app={controls.app} reducedMotion={() => false} clock={Date.now} createEmulator={fakeEmulators().factory} />);

  return controls;
};

const rail = () => screen.findByRole("region", { name: "rail" });

afterEach(cleanup);

describe("u26 the harness seeds", () => {
  it("u26_the_seeds_include_the_first_room_screens_the_recipe_documents", () => {
    expect(SEEDS).toEqual(["first-run", "agents-10", "tree-40", "daemon-exits", "conflict", "empty-project", "door-stopped", "decisions", "earlier-run"]);
  });

  it("u26_first_run_shows_the_empty_rail_text", async () => {
    mount("first-run");

    await screen.findByText("open a folder to start");

    expect((await rail()).textContent).toBe("agents and terminalsappear here, one per row,nested by indent");
  });

  it("u143_the_empty_project_seed_exposes_the_first_room_action", async () => {
    mount("empty-project");

    expect(await screen.findByRole("button", { name: "start a Room" })).toBeTruthy();
    expect(screen.queryByText("open a folder to start")).toBeNull();
  });

  it("u143_the_stopped_door_seed_shows_its_exit_and_restart_action", async () => {
    mount("door-stopped");
    fireEvent.click(await screen.findByText("first room"));

    expect(await screen.findByText("Door stopped, exited 0")).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "start Door" }).length).toBeGreaterThan(0);
  });

  it("u83_the_earlier_run_seed_reads_the_line_for_its_selected_agent", async () => {
    const { app } = mount("earlier-run");
    fireEvent.click(await screen.findByText("earlier agent"));

    expect(await screen.findByText("no output kept from an earlier run")).toBeTruthy();
    expect(app.calls.filter((call) => call.method === "terminal.write" || call.method === "terminal.resize")).toEqual([]);
  });

  it("u26_agents_10_shows_its_first_and_last_agent_in_the_rail", async () => {
    mount("agents-10");

    const region = within(await rail());

    expect(await region.findByText("agent-1")).toBeTruthy();
    expect(region.getByText("agent-10")).toBeTruthy();
  });

  it("u26_tree_40_shows_an_agent_two_groups_deep_and_its_last_agent_in_the_rail", async () => {
    mount("tree-40");

    const region = within(await rail());

    expect(await region.findByText("token-rotation")).toBeTruthy();
    expect(region.getByText("agent-31")).toBeTruthy();
  });

  it("u26_daemon_exits_shows_the_exit_in_the_centre", async () => {
    mount("daemon-exits");

    const line = await screen.findByRole("alert");

    expect(line.textContent).toBe("daemon exited 1");
    expect(line.querySelector("svg.lucide-x")).not.toBeNull();
  });

  it("u26_conflict_fails_the_next_call_with_conflict_and_the_one_after_succeeds", async () => {
    const { app } = mount("conflict");
    await within(await rail()).findByText("agent-1");
    await new Promise((resolve) => setTimeout(resolve, 0));

    await expect(app.rpc("rail.createRoom", { name: "x", parent: null })).rejects.toMatchObject({ code: -32003 });
    await expect(app.rpc("rail.createRoom", { name: "x", parent: null })).resolves.toMatchObject({ name: "x" });
  });

  it("u26_a_pad_the_agent_owns_takes_an_append_and_shows_the_new_text", async () => {
    mount("tree-40");

    // U58: agent-1 is on the Rail, so its Pads show under it, not in the Shelf.
    const owner = await within(await rail()).findByText("agent-1");
    fireEvent.click(await within(owner.closest<HTMLElement>("[role=treeitem]")!).findByRole("button", { name: /pads$/ }));
    fireEvent.click(await screen.findByText("pad-0"));
    const append = await screen.findByLabelText<HTMLInputElement>("append");
    fireEvent.input(append, { target: { value: "appended line" } });
    fireEvent.keyDown(append, { key: "Enter" });

    await waitFor(() => expect(screen.getByLabelText("Pad body").textContent).toContain("appended line"));
  });

  it("u26_set_status_moves_the_row_to_the_new_label", async () => {
    const { setStatus } = mount("tree-40");
    await within(await rail()).findByText("agent-3");

    setStatus("agent-3", "needs-you", "asks: keep v1?");

    expect(await within(await rail()).findByText(/asks: keep v1\?/)).toBeTruthy();
  });
});

describe("u26 the controls", () => {
  const listen = async (seed: SeedName) => {
    const controls = seedApp(seed, NOW);
    const seen: DaemonEvent[] = [];
    await controls.app.subscribe((received) => seen.push(received));

    return { ...controls, seen };
  };

  it("u26_write_output_sends_terminal_output_with_the_text_in_base64", async () => {
    const { writeOutput, seen } = await listen("agents-10");

    writeOutput("t-agent-1", "hi");

    expect(seen).toEqual([event({ name: "terminal.output", data: { id: "t-agent-1", offset: 0, data: "aGk=" } })]);
  });

  it("u103_the_fake_daemon_snapshot_matches_the_output_offsets_after_a_burst", async () => {
    const { app, writeOutput, seen } = await listen("agents-10");

    writeOutput("t-agent-1", "one ");
    writeOutput("t-agent-1", "é");
    const snapshot = await app.rpc("terminal.snapshot", { id: "t-agent-1" });

    expect([snapshot.after, new TextDecoder().decode(fromBase64(snapshot.data))]).toEqual([6, "one é"]);
    expect(seen.map((item) => item.name === "terminal.output" ? item.data.offset : null)).toEqual([0, 4]);
    await expect(app.rpc("terminal.snapshot", { id: "absent" })).rejects.toMatchObject({ code: -32001 });
  });

  it("u26_emit_sends_the_event_it_is_given", async () => {
    const { emit, seen } = await listen("agents-10");

    emit(event({ name: "rail.changed" }));

    expect(seen).toEqual([event({ name: "rail.changed" })]);
  });

  it("u26_fail_next_rejects_one_call_with_the_given_code_and_message", async () => {
    const { app, failNext } = await listen("agents-10");

    failNext(-32001, "gone");

    await expect(app.rpc("rail.tree", null)).rejects.toMatchObject({ code: -32001, message: "gone" });
    await expect(app.rpc("rail.tree", null)).resolves.toHaveLength(10);
  });
});

describe("u26 the Daemon's rail methods", () => {
  const daemon = async () => {
    const controls = seedApp("tree-40", NOW);
    const seen: DaemonEvent[] = [];
    await controls.app.subscribe((received) => seen.push(received));
    const names = async () => (await controls.app.rpc("rail.tree", null)).map((node) => node.name);
    const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

    return { app: controls.app, seen, names, tick };
  };

  it("u26_spawning_an_agent_adds_a_node_and_sends_rail_changed", async () => {
    const { app, seen, names, tick } = await daemon();

    const spawned = await app.rpc("agent.spawn", { cwd: "/p", prompt: null, parent: "backend" });
    await tick();

    expect([(await names()).length, spawned.parent, seen]).toEqual([41, "backend", [event({ name: "rail.changed" })]]);
  });

  it("u26_spawning_a_terminal_adds_a_terminal_under_the_parent", async () => {
    const { app } = await daemon();

    const spawned = await app.rpc("rail.spawnTerminal", { cwd: "/p", parent: "auth" });

    expect([spawned.kind, spawned.parent]).toEqual(["terminal", "auth"]);
  });

  it("u26_creating_a_group_adds_a_group_with_the_name", async () => {
    const { app, names } = await daemon();

    await app.rpc("rail.createRoom", { name: "infra", parent: null });

    expect(await names()).toContain("infra");
  });

  it("u26_renaming_a_node_changes_its_name_in_the_tree", async () => {
    const { app, names } = await daemon();

    await app.rpc("rail.rename", { id: "backend", name: "api" });

    expect(await names()).toContain("api");
  });

  it("u26_promoting_a_group_makes_it_a_meta_agent_with_a_terminal", async () => {
    const { app } = await daemon();

    const promoted = await app.rpc("rail.startDoor", { id: "backend" });

    expect([promoted.kind, promoted.terminal_id]).toEqual(["room", "t-backend-1"]);
  });

  it("u26_moving_a_node_re_parents_it_at_the_index", async () => {
    const { app } = await daemon();

    await app.rpc("rail.move", { id: "migrate", parent: "auth", index: 0 });
    const moved = (await app.rpc("rail.tree", null)).find((node) => node.id === "migrate");

    expect([moved?.parent, moved?.order]).toEqual(["auth", 0]);
  });

  it("u26_moving_a_node_to_a_later_index_puts_it_after_the_siblings_before_that_index", async () => {
    const { app } = await daemon();

    await app.rpc("rail.move", { id: "docs", parent: "backend", index: 2 });

    const order = (await app.rpc("rail.tree", null))
      .filter((node) => node.parent === "backend")
      .sort((left, right) => left.order - right.order)
      .map((node) => node.id);

    expect(order).toEqual(["auth", "migrate", "docs", "shell"]);
  });

  it("u26_killing_a_terminal_marks_it_exited_and_sends_terminal_exited", async () => {
    const { app, seen } = await daemon();

    await app.rpc("terminal.kill", { id: "t-shell" });
    const listed = (await app.rpc("terminal.list", null)).find((info) => info.id === "t-shell");

    expect([listed?.running, seen]).toEqual([false, [event({ name: "terminal.exited", data: { id: "t-shell", code: null } })]]);
  });

  it("u26_plain_terminal_snapshots_have_no_agent_attempt_or_revision", async () => {
    const { app } = await daemon();
    const terminals = (await app.rpc("rail.tree", null)).filter((node) => node.kind === "terminal");
    expect(terminals.length).toBeGreaterThan(0);

    for (const node of terminals) {
      expect([node.status, node.attempt, node.status_revision]).toEqual([null, null, null]);
    }
  });

  it("u26_stop_advances_the_status_revision_once_and_repeated_stop_is_inert", async () => {
    const { app, seen } = await daemon();
    const before = (await app.rpc("rail.tree", null)).find((node) => node.id === "tokens")!;
    await app.rpc("agent.stop", { id: "tokens" });
    const stopped = (await app.rpc("rail.tree", null)).find((node) => node.id === "tokens")!;
    expect(stopped.attempt).toBe(before.attempt);
    expect(stopped.status_revision).toBe(String(BigInt(before.status_revision!) + 1n));
    expect(stopped.status?.kind).toBe("done");
    seen.length = 0;
    await app.rpc("agent.stop", { id: "tokens" });
    expect((await app.rpc("rail.tree", null)).find((node) => node.id === "tokens")).toEqual(stopped);
    expect(seen).toEqual([]);
  });

  it("u26_stopping_an_agent_exits_its_terminal", async () => {
    const { app } = await daemon();

    await app.rpc("agent.stop", { id: "tokens" });
    const listed = (await app.rpc("terminal.list", null)).find((info) => info.id === "t-tokens");

    expect(listed?.running).toBe(false);
  });

  it("u26_writing_to_and_resizing_a_terminal_answer_null", async () => {
    const { app } = await daemon();

    expect([
      await app.rpc("terminal.write", { id: "t-shell", data: "bHM=" }),
      await app.rpc("terminal.resize", { id: "t-shell", cols: 80, rows: 24 }),
    ]).toEqual([null, null]);
  });

  it("u26_a_node_that_is_not_in_the_tree_fails_with_not_found", async () => {
    const { app } = await daemon();

    await expect(app.rpc("rail.rename", { id: "nope", name: "x" })).rejects.toMatchObject({ code: -32001 });
  });
});
