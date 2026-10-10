import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import type { Actor } from "@contracts/Actor";
import type { RailNode } from "@contracts/agent/RailNode";
import type { Pad } from "@contracts/pad/Pad";
import { DrawerHost } from "../../drawer/DrawerHost";
import { Keys } from "../../keys/Keys";
import { Pads } from "../../pads/Pads";
import { ConnectedProjectContext, connectProject } from "../../state/connectedProject";
import { createFakeApp } from "../../testing/fakeApp";
import { agent, event, NOW } from "../../testing/nodes";
import { padHandlers } from "../../testing/stores";
import type { PadStore } from "../../testing/stores";
import { Rail } from "../Rail";
import { rowOf } from "../railFixture";

afterEach(cleanup);

const owner = (id: string): Actor => ({ kind: "agent", id, parent: null });

const USER: Actor = { kind: "user", id: "you", parent: null };

const padOf = (name: string, who: Actor): Pad => ({ name, owner: who, text: "", updated_at: 0 });

/** The Rail, `Keys`, the Shelf and the Drawer over one fake Daemon whose Pads are `pads`. */
const mount = async (tree: RailNode[], pads: Pad[]) => {
  const state: PadStore = { pads, history: [] };
  const app = createFakeApp();
  app.handlers["rail.tree"] = () => tree;
  Object.assign(app.handlers, padHandlers(state, (data) => app.emit(event(data))));

  const connected = await connectProject(app, { name: "p", path: "/p" }, () => true, () => NOW);

  render(() => (
    <ConnectedProjectContext.Provider value={connected}>
      <Keys />
      <Rail />
      <Pads />
      <DrawerHost drawer={connected.drawer} reducedMotion={() => true} />
    </ConnectedProjectContext.Provider>
  ));

  await connected.rail.settled();

  return { app, state, connected };
};

const TREE = [agent("a1", "working", "x", { name: "auth" }), agent("a2", "working", "x", { name: "docs", order: 1 })];

const control = (row: HTMLElement) => within(row).queryByRole("button", { name: /pads$/ });

const shelf = () => within(screen.getByRole("region", { name: "pads" }));

describe("u58 an Agent's Pads under it", () => {
  it("u58_the_control_counts_the_agents_pads_and_is_absent_at_zero", async () => {
    await mount(TREE, [padOf("n1", owner("a1")), padOf("n2", owner("a1")), padOf("u", USER)]);

    await waitFor(() => expect(control(rowOf("auth"))?.textContent?.trim()).toBe("2"));
    expect(control(rowOf("docs"))).toBeNull();
  });

  it("u58_the_control_is_collapsed_at_rest_and_a_click_shows_and_folds_the_pads_one_level_deeper", async () => {
    await mount(TREE, [padOf("notes", owner("a1"))]);

    const button = await waitFor(() => control(rowOf("auth")) ?? Promise.reject(new Error("no control")));

    expect(button.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByTitle("notes")).toBeNull();

    fireEvent.click(button);

    const pad = screen.getByTitle("notes");

    expect(button.getAttribute("aria-expanded")).toBe("true");
    expect(pad.closest<HTMLElement>("[data-pads]")?.style.paddingLeft).toBe("calc(1 * var(--space-4))");
    expect(rowOf("auth").nextElementSibling).toBe(pad.closest("[data-pads]"));

    fireEvent.click(button);

    expect(screen.queryByTitle("notes")).toBeNull();
  });

  it("u58_the_control_does_not_select_the_row_or_reorder_the_agents", async () => {
    const { connected } = await mount(TREE, [padOf("notes", owner("a1"))]);

    fireEvent.click(await waitFor(() => control(rowOf("auth")) ?? Promise.reject(new Error("no control"))));

    expect(connected.rail.selected()).toBeNull();
    expect([...document.querySelectorAll("[role=tree] [data-id]")].map((row) => row.getAttribute("data-id"))).toEqual(["a1", "a2"]);
  });

  it("u58_the_shelf_holds_only_users_pads_and_pads_of_agents_off_the_rail", async () => {
    await mount(TREE, [padOf("mine", USER), padOf("theirs", owner("a1")), padOf("orphan", owner("gone"))]);

    await shelf().findByText("mine");

    expect(shelf().getByText("orphan")).toBeTruthy();
    expect(shelf().queryByText("theirs")).toBeNull();
  });

  it("u58_a_click_on_a_pad_opens_its_drawer_and_leaves_the_selection", async () => {
    const { connected } = await mount(TREE, [padOf("notes", owner("a1"))]);

    fireEvent.click(await waitFor(() => control(rowOf("auth")) ?? Promise.reject(new Error("no control"))));
    fireEvent.click(screen.getByTitle("notes"));

    const drawer = await screen.findByLabelText("drawer");

    expect(await within(drawer).findByText("notes")).toBeTruthy();
    expect(connected.rail.selected()).toBeNull();
  });

  it("u58_clicking_the_owner_mark_in_the_drawer_calls_pad_setOwner_and_moves_the_pad_to_the_shelf", async () => {
    const { app } = await mount(TREE, [padOf("notes", owner("a1"))]);

    fireEvent.click(await waitFor(() => control(rowOf("auth")) ?? Promise.reject(new Error("no control"))));
    fireEvent.click(screen.getByTitle("notes"));
    fireEvent.click(await within(await screen.findByLabelText("drawer")).findByRole("button", { name: "make yours" }));

    await shelf().findByText("notes");

    expect(app.calls.filter((call) => call.method === "pad.setOwner")).toEqual([
      { method: "pad.setOwner", params: { name: "notes", owner: USER } },
    ]);
    expect(control(rowOf("auth"))).toBeNull();
  });

  it("u58_the_rail_and_the_shelf_share_one_pad_list", async () => {
    const { app } = await mount(TREE, [padOf("notes", owner("a1"))]);

    await waitFor(() => expect(control(rowOf("auth"))).not.toBeNull());

    expect(app.calls.filter((call) => call.method === "pad.list")).toHaveLength(1);
  });

  it("u58_right_on_a_leaf_agent_shows_its_pads_and_left_folds_them_before_it_selects_the_parent", async () => {
    await mount(TREE, [padOf("notes", owner("a1"))]);
    await waitFor(() => expect(control(rowOf("auth"))).not.toBeNull());

    const row = rowOf("auth");

    row.focus();
    fireEvent.keyDown(row, { key: "ArrowRight" });
    expect(screen.getByTitle("notes")).toBeTruthy();

    fireEvent.keyDown(row, { key: "ArrowLeft" });
    expect(screen.queryByTitle("notes")).toBeNull();
    expect(document.activeElement).toBe(row);
  });
});
