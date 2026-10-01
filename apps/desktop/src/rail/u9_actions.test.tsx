import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import type { RailNode } from "@contracts/agent/RailNode";
import { RpcError } from "../app/seam";
import { agent, callsTo, event, glyphOf, group, metaAgent, mountRail, rowNames, rowOf, terminal } from "./railFixture";

afterEach(cleanup);

const SPAWNED = agent("fresh", "idle", "starting");

type Mounted = Awaited<ReturnType<typeof mountRail>>;

const answerWith = (mounted: Mounted, method: "agent.spawn" | "rail.spawnTerminal" | "rail.createGroup") => {
  mounted.app.handlers[method] = () => SPAWNED;
};

describe("u9 actions", () => {
  it("u9_plus_agent_spawns_at_the_top_level_with_no_selection", async () => {
    const mounted = await mountRail([group("g")]);
    answerWith(mounted, "agent.spawn");

    fireEvent.click(screen.getByText("+ agent"));

    await waitFor(() => expect(callsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: null, parent: null }]));
  });

  it("u9_plus_agent_spawns_under_the_selected_group", async () => {
    const mounted = await mountRail([group("g")]);
    answerWith(mounted, "agent.spawn");
    mounted.rail.select("g");

    fireEvent.click(screen.getByText("+ agent"));

    await waitFor(() => expect(callsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: null, parent: "g" }]));
  });

  it("u9_plus_agent_spawns_under_the_selected_meta_agent", async () => {
    const mounted = await mountRail([metaAgent("lead", "idle", "i")]);
    answerWith(mounted, "agent.spawn");
    mounted.rail.select("lead");

    fireEvent.click(screen.getByText("+ agent"));

    await waitFor(() =>
      expect(callsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: null, parent: "lead" }]),
    );
  });

  it.each<[string, RailNode]>([
    ["an agent", agent("a", "idle", "i")],
    ["a terminal", terminal("a")],
  ])("u9_plus_agent_with_%s_selected_spawns_at_the_top_level", async (_name, selected) => {
    const mounted = await mountRail([selected]);
    answerWith(mounted, "agent.spawn");
    mounted.rail.select("a");

    fireEvent.click(screen.getByText("+ agent"));

    await waitFor(() => expect(callsTo(mounted.app, "agent.spawn")).toEqual([{ cwd: "/p", prompt: null, parent: null }]));
  });

  it("u9_the_new_agents_row_becomes_selected_once_the_tree_has_it", async () => {
    const tree: RailNode[] = [group("g")];
    const mounted = await mountRail(tree);
    mounted.app.handlers["agent.spawn"] = () => {
      tree.push(SPAWNED);
      mounted.app.emit(event({ name: "rail.changed" }));

      return SPAWNED;
    };

    fireEvent.click(screen.getByText("+ agent"));

    await waitFor(() => expect(mounted.rail.selected()).toBe("fresh"));
  });

  it("u9_the_webview_never_adds_a_row_the_tree_does_not_have", async () => {
    const mounted = await mountRail([group("g")]);
    answerWith(mounted, "agent.spawn");

    fireEvent.click(screen.getByText("+ agent"));
    await waitFor(() => expect(callsTo(mounted.app, "agent.spawn")).toHaveLength(1));

    expect([rowNames(), mounted.rail.selected()]).toEqual([["g"], null]);
  });

  it("u9_plus_terminal_spawns_a_terminal_under_the_selected_group", async () => {
    const mounted = await mountRail([group("g")]);
    answerWith(mounted, "rail.spawnTerminal");
    mounted.rail.select("g");

    fireEvent.click(screen.getByText("+ terminal"));

    await waitFor(() => expect(callsTo(mounted.app, "rail.spawnTerminal")).toEqual([{ cwd: "/p", parent: "g" }]));
  });

  it("u9_plus_terminal_spawns_at_the_top_level_with_no_selection", async () => {
    const mounted = await mountRail([group("g")]);
    answerWith(mounted, "rail.spawnTerminal");

    fireEvent.click(screen.getByText("+ terminal"));

    await waitFor(() => expect(callsTo(mounted.app, "rail.spawnTerminal")).toEqual([{ cwd: "/p", parent: null }]));
  });

  it("u9_plus_group_creates_a_group_named_group_under_the_selected_group", async () => {
    const mounted = await mountRail([group("g")]);
    answerWith(mounted, "rail.createGroup");
    mounted.rail.select("g");

    fireEvent.click(screen.getByText("+ group"));

    await waitFor(() => expect(callsTo(mounted.app, "rail.createGroup")).toEqual([{ name: "group", parent: "g" }]));
  });

  it("u9_double_clicking_a_name_edits_it_and_enter_renames", async () => {
    const mounted = await mountRail([agent("a", "idle", "i")]);
    mounted.app.handlers["rail.rename"] = () => agent("a", "idle", "i", { name: "renamed" });

    fireEvent.dblClick(screen.getByText("a"));
    const field = screen.getByLabelText("name");
    fireEvent.input(field, { target: { value: "renamed" } });
    fireEvent.keyDown(field, { key: "Enter" });

    await waitFor(() => expect(callsTo(mounted.app, "rail.rename")).toEqual([{ id: "a", name: "renamed" }]));
  });

  it("u9_escape_keeps_the_old_name_and_calls_nothing", async () => {
    const mounted = await mountRail([agent("a", "idle", "i")]);

    fireEvent.dblClick(screen.getByText("a"));
    const field = screen.getByLabelText("name");
    fireEvent.input(field, { target: { value: "renamed" } });
    fireEvent.keyDown(field, { key: "Escape" });

    expect([callsTo(mounted.app, "rail.rename"), screen.queryByLabelText("name"), rowNames()]).toEqual([[], null, ["a"]]);
  });

  it("u9_an_empty_name_keeps_the_old_name_and_calls_nothing", async () => {
    const mounted = await mountRail([agent("a", "idle", "i")]);

    fireEvent.dblClick(screen.getByText("a"));
    const field = screen.getByLabelText("name");
    fireEvent.input(field, { target: { value: "   " } });
    fireEvent.keyDown(field, { key: "Enter" });

    expect([callsTo(mounted.app, "rail.rename"), screen.queryByLabelText("name")]).toEqual([[], null]);
  });

  it("u9_hovering_a_plain_group_shows_promote_which_calls_rail_promote", async () => {
    const mounted = await mountRail([group("g")]);
    mounted.app.handlers["rail.promote"] = () => metaAgent("g", "idle", "i");

    fireEvent.mouseEnter(rowOf("g"));
    fireEvent.click(screen.getByText("promote"));

    await waitFor(() => expect(callsTo(mounted.app, "rail.promote")).toEqual([{ id: "g" }]));
  });

  it("u9_promote_is_hidden_until_hover_and_never_on_an_agent", async () => {
    await mountRail([group("g"), agent("a", "idle", "i")]);
    const before = screen.queryByText("promote");

    fireEvent.mouseEnter(rowOf("a"));

    expect([before, screen.queryByText("promote")]).toEqual([null, null]);
  });

  it("u9_a_failed_call_shows_one_ink_line_until_the_next_click", async () => {
    const mounted = await mountRail([group("g")]);
    mounted.app.handlers["agent.spawn"] = () => {
      throw new RpcError(-32000, "no claude on PATH");
    };

    fireEvent.click(screen.getByText("+ agent"));
    const shown = await screen.findByText("✕ no claude on PATH");
    fireEvent.click(rowOf("g"));

    expect([shown.className, screen.queryByText("✕ no claude on PATH")]).toEqual(["ink", null]);
  });

  it("u9_the_failure_line_sits_above_the_actions", async () => {
    const mounted = await mountRail([]);
    mounted.app.handlers["rail.createGroup"] = () => {
      throw new RpcError(-32000, "boom");
    };

    fireEvent.click(screen.getByText("+ group"));
    const line = await screen.findByText("✕ boom");

    expect(line.compareDocumentPosition(screen.getByText("+ agent")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("u9_the_failure_line_clears_on_a_click_on_the_fold_triangle", async () => {
    const mounted = await mountRail([group("g")]);
    mounted.app.handlers["rail.createGroup"] = () => {
      throw new RpcError(-32000, "boom");
    };

    fireEvent.click(screen.getByText("+ group"));
    await screen.findByText("✕ boom");

    fireEvent.click(glyphOf("g"));

    expect(screen.queryByText("✕ boom")).toBeNull();
  });

  it("u9_the_failure_line_clears_on_a_click_on_promote", async () => {
    const mounted = await mountRail([group("g")]);
    mounted.app.handlers["rail.createGroup"] = () => {
      throw new RpcError(-32000, "boom");
    };

    mounted.app.handlers["rail.promote"] = () => metaAgent("g", "idle", "i");

    fireEvent.click(screen.getByText("+ group"));
    await screen.findByText("✕ boom");
    fireEvent.mouseEnter(rowOf("g"));

    fireEvent.click(screen.getByText("promote"));

    expect(screen.queryByText("✕ boom")).toBeNull();
  });

  it("u9_the_failure_line_clears_on_a_click_outside_the_rail", async () => {
    const mounted = await mountRail([]);
    mounted.app.handlers["rail.createGroup"] = () => {
      throw new RpcError(-32000, "boom");
    };

    fireEvent.click(screen.getByText("+ group"));
    await screen.findByText("✕ boom");

    fireEvent.click(document.body);

    expect(screen.queryByText("✕ boom")).toBeNull();
  });

  it("u9_promote_is_not_shown_on_a_meta_agent", async () => {
    await mountRail([metaAgent("lead", "idle", "i")]);

    fireEvent.mouseEnter(rowOf("lead"));

    expect(screen.queryByText("promote")).toBeNull();
  });

  it("u9_a_collapsed_group_can_still_be_promoted", async () => {
    const mounted = await mountRail([group("g"), agent("a", "idle", "i", { parent: "g" })]);
    mounted.app.handlers["rail.promote"] = () => metaAgent("g", "idle", "i");
    fireEvent.click(glyphOf("g"));

    fireEvent.mouseEnter(rowOf("g"));
    fireEvent.click(screen.getByText("promote"));

    await waitFor(() => expect(callsTo(mounted.app, "rail.promote")).toEqual([{ id: "g" }]));
  });

  it("u9_a_double_click_on_plus_agent_spawns_one_agent", async () => {
    const mounted = await mountRail([]);
    let finish: (node: RailNode) => void = () => {};

    mounted.app.handlers["agent.spawn"] = () => new Promise<RailNode>((done) => (finish = done));
    const button = screen.getByText("+ agent");

    fireEvent.click(button);
    fireEvent.click(button);
    finish(SPAWNED);

    await waitFor(() => expect(button).toHaveProperty("disabled", false));
    expect(callsTo(mounted.app, "agent.spawn")).toHaveLength(1);
  });
});
