import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import type { RailNode } from "@contracts/agent/RailNode";
import { RpcError } from "../app/seam";
import { agent, door, event, workstream } from "../testing/nodes";
import { mountRailAndPane } from "./paneHarness";

afterEach(cleanup);

const startWords = () => screen.getAllByText(/^(start|retry) Door$/);

const paneStart = () => startWords().find((word) => word.closest(".pane-door"))!;

const calls = (app: { calls: { method: string; params: unknown }[] }, method: string) => app.calls.filter((call) => call.method === method).map((call) => call.params);

const startWorkstream = () => screen.getByText("new Workstream", { selector: ".pane-action" });

const emptyProject = async () => {
  const tree: RailNode[] = [];
  const mounted = await mountRailAndPane(tree);
  mounted.app.handlers["rail.tree"] = () => structuredClone(tree);
  mounted.app.handlers["rail.createWorkstream"] = () => {
    const created = workstream("made");
    tree.push(created);
    mounted.app.emit(event({ name: "rail.changed" }));

    return created;
  };

  return { ...mounted, tree };
};

describe("u143 empty Project", () => {
  it("u143_an_empty_project_offers_start_a_workstream_in_place_of_the_select_line", async () => {
    await emptyProject();

    expect([screen.getByText("no Workstream yet"), startWorkstream().hasAttribute("aria-disabled"), screen.queryByText("select an agent or a terminal")]).toEqual([expect.anything(), false, null]);
  });

  it("u143_the_start_a_workstream_button_is_a_native_tab_stop", async () => {
    await emptyProject();
    startWorkstream().focus();

    expect([startWorkstream().tagName, startWorkstream().getAttribute("tabindex"), document.activeElement === startWorkstream()]).toEqual(["BUTTON", null, true]);
  });

  it("u143_only_new_workstream_is_offered_on_an_empty_project", async () => {
    await emptyProject();

    expect([...document.querySelectorAll(".rail-actions button")].map((button) => button.textContent?.trim())).toEqual(["new Workstream"]);
    expect(screen.queryByText("agent", { selector: "button" })).toBeNull();
  });

  it("u143_with_rows_and_nothing_selected_the_select_line_stays_and_no_workstream_button_shows", async () => {
    await mountRailAndPane([agent("a", "idle", "i")]);

    expect([screen.queryByText("select an agent or a terminal") !== null, screen.queryByText("new Workstream", { selector: ".pane-action" })]).toEqual([true, null]);
  });

  it("u143_start_a_workstream_creates_one_root_workstream_selects_it_and_starts_only_its_door", async () => {
    const { app, connected } = await emptyProject();
    app.handlers["rail.startDoor"] = () => door("made", "working", "starting");

    fireEvent.click(startWorkstream());

    await waitFor(() => expect(calls(app, "rail.startDoor")).toEqual([{ id: "made" }]));
    expect([calls(app, "rail.createWorkstream"), connected.rail.selected()]).toEqual([[{ name: "workstream", parent: null }], "made"]);
  });

  it("u143_a_pending_creation_reads_creating_and_repeat_activation_calls_nothing", async () => {
    const { app } = await emptyProject();
    app.handlers["rail.createWorkstream"] = () => new Promise<RailNode>(() => {});

    fireEvent.click(startWorkstream());
    await screen.findByText("creating Workstream…");
    fireEvent.click(startWorkstream());
    fireEvent.click(screen.getByText("new Workstream", { selector: ".rail-actions button" }));

    expect([calls(app, "rail.createWorkstream").length, startWorkstream().hasAttribute("aria-disabled"), screen.queryByText("no Workstream yet")]).toEqual([1, true, null]);
  });

  it("u143_a_failed_creation_shows_the_message_keeps_the_button_and_selects_nothing", async () => {
    const { app, connected } = await emptyProject();
    app.handlers["rail.createWorkstream"] = () => {
      throw new RpcError(-32000, "no space");
    };

    fireEvent.click(startWorkstream());

    expect([(await screen.findAllByRole("alert")).map((line) => line.textContent), startWorkstream().hasAttribute("aria-disabled"), connected.rail.selected(), calls(app, "rail.startDoor")]).toEqual([["no space", "no space"], false, null, []]);
  });

  it("u143_a_failed_creation_can_be_tried_again_from_the_same_button", async () => {
    const { app } = await emptyProject();
    const created = app.handlers["rail.createWorkstream"]!;
    app.handlers["rail.createWorkstream"] = () => {
      app.handlers["rail.createWorkstream"] = created;
      throw new RpcError(-32000, "no space");
    };

    app.handlers["rail.startDoor"] = () => door("made", "working", "starting");

    fireEvent.click(startWorkstream());
    await screen.findAllByRole("alert");
    fireEvent.click(startWorkstream());

    await waitFor(() => expect(calls(app, "rail.startDoor")).toEqual([{ id: "made" }]));
    expect(calls(app, "rail.createWorkstream").length).toBe(2);
  });

  it("u143_a_failed_door_launch_retries_the_same_workstream_without_creating_another", async () => {
    const { app } = await emptyProject();
    app.handlers["rail.startDoor"] = () => {
      throw new RpcError(-32003, "launch failed");
    };

    fireEvent.click(startWorkstream());
    await screen.findByText("Door did not start");
    app.handlers["rail.startDoor"] = () => door("made", "working", "starting");
    fireEvent.click(paneStart());

    await waitFor(() => expect(calls(app, "rail.startDoor")).toEqual([{ id: "made" }, { id: "made" }]));
    expect(calls(app, "rail.createWorkstream").length).toBe(1);
  });
});

describe("u143 a selected Workstream whose Door is not running", () => {
  it("u143_a_workstream_never_launched_reads_not_started_and_launches_nothing", async () => {
    const { app, connected } = await mountRailAndPane([workstream("r")]);

    connected.rail.select("r");

    expect([await screen.findByText("Door not started"), calls(app, "rail.startDoor")]).toEqual([expect.anything(), []]);
  });

  it("u143_a_door_that_exited_reads_stopped_with_its_code_and_start_runs_it_once", async () => {
    const tree: RailNode[] = [door("r", "working", "w")];
    const { app, connected } = await mountRailAndPane(tree);

    connected.rail.select("r");
    app.emit(event({ name: "terminal.exited", data: { id: "t-r", code: 137 } }));
    await screen.findByText("Door stopped, exited 137");
    fireEvent.click(paneStart());

    await waitFor(() => expect(calls(app, "rail.startDoor")).toEqual([{ id: "r" }]));
  });

  it("u143_a_running_door_shows_no_state_words", async () => {
    const { connected } = await mountRailAndPane([door("r", "working", "w")]);

    connected.rail.select("r");

    expect(screen.queryByText(/^Door (not started|stopped|did not start)|^starting Door/)).toBeNull();
  });

  it("u143_a_pending_start_reads_starting_and_cannot_be_activated_again", async () => {
    const { app, connected } = await mountRailAndPane([workstream("r")]);
    app.handlers["rail.startDoor"] = () => new Promise<RailNode>(() => {});

    connected.rail.select("r");
    fireEvent.click(await screen.findByText("start Door", { selector: ".pane-door *" }));
    await screen.findByText("starting Door…");
    fireEvent.click(paneStart());

    expect([calls(app, "rail.startDoor").length, paneStart().hasAttribute("aria-disabled"), screen.queryByText("Door not started")]).toEqual([1, true, null]);
  });

  it("u143_a_failed_start_reads_did_not_start_with_the_message_and_retry", async () => {
    const { app, connected } = await mountRailAndPane([workstream("r")]);
    app.handlers["rail.startDoor"] = () => {
      throw new RpcError(-32003, "launch failed");
    };

    connected.rail.select("r");
    fireEvent.click(await screen.findByText("start Door", { selector: ".pane-door *" }));

    expect([await screen.findByText("Door did not start"), (await screen.findAllByRole("alert")).map((line) => line.textContent), paneStart().textContent]).toEqual([expect.anything(), ["launch failed", "launch failed"], "retry Door"]);
  });
});

describe("u143 direct Agent access beside a stopped Door", () => {
  it("u143_agent_still_spawns_inside_the_selected_stopped_workstream", async () => {
    const { app, connected } = await mountRailAndPane([workstream("r")]);

    connected.rail.select("r");
    fireEvent.click(screen.getByText("agent"));

    await waitFor(() => expect(calls(app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: null, parent: "r" }]));
  });
});
