import { cleanup, fireEvent, render, screen } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import { afterEach, describe, expect, it } from "vitest";
import type { Actor } from "@contracts/Actor";
import type { Touch } from "@contracts/Touch";
import { Layout } from "../app/Layout";
import { createFakeApp } from "../testing/fakeApp";
import { WorkspaceContext, openWorkspace } from "../state/workspace";
import { createDrawer } from "./drawer";
import { DrawerHost } from "./DrawerHost";
import { LastTouch } from "./LastTouch";
import { lastTouchLine } from "./touchLine";

const USER: Actor = { kind: "user", id: "you", parent: null };

const AGENT: Actor = { kind: "agent", id: "a", parent: null };

const names = (actor: Actor) => (actor.kind === "agent" ? "auth-refactor" : "you");

const at = (hours: number, minutes: number) => new Date(2026, 0, 5, hours, minutes).getTime();

const touch = (actor: Actor, verb: Touch["verb"], when: number): Touch => ({ actor, verb, item: "todo:3", at: when });

const drawerSetup = (reduced = false) => {
  const drawer = createDrawer();
  const [reducedMotion, setReducedMotion] = createSignal(reduced);
  render(() => <DrawerHost drawer={drawer} reducedMotion={reducedMotion} />);

  return { drawer, setReducedMotion, panel: screen.getByLabelText("drawer") };
};

afterEach(cleanup);

describe("u5 Drawer and last touch", () => {
  it("u5_a_closed_drawer_waits_off_screen_to_the_right", () => {
    const { panel } = drawerSetup();

    expect(panel.style.transform).toBe("translateX(100%)");
  });

  it("u5_an_open_drawer_slides_in_from_the_right_in_180_ms_ease_out", () => {
    const { drawer, panel } = drawerSetup();

    drawer.open(() => <p>detail</p>);

    expect([panel.style.transform, panel.style.transition]).toEqual([
      "translateX(0)",
      "transform 180ms ease-out",
    ]);
  });

  it("u5_under_prefers_reduced_motion_the_drawer_is_instant", () => {
    const { drawer, panel } = drawerSetup(true);

    drawer.open(() => <p>detail</p>);

    expect(panel.style.transition).toBe("none");
  });

  it("u5_the_drawer_shows_a_light_close_word_that_closes_it", () => {
    const { drawer } = drawerSetup();
    drawer.open(() => <p>detail</p>);

    fireEvent.click(screen.getByText("close"));

    expect(drawer.content()).toBeNull();
  });

  it("u5_opening_another_drawer_replaces_the_open_one", () => {
    const { drawer } = drawerSetup();
    drawer.open(() => <p>first</p>);

    drawer.open(() => <p>second</p>);

    expect([screen.queryByText("first"), screen.queryByText("second")].map(Boolean)).toEqual([false, true]);
  });

  it("u5_a_closed_drawer_unmounts_its_content_when_the_slide_out_ends", () => {
    const { drawer, panel } = drawerSetup();
    drawer.open(() => <p>detail</p>);
    drawer.close();

    fireEvent.transitionEnd(panel);

    expect(screen.queryByText("detail")).toBeNull();
  });

  it("u5_a_closed_drawer_unmounts_its_content_at_once_under_reduced_motion", () => {
    const { drawer } = drawerSetup(true);
    drawer.open(() => <p>detail</p>);

    drawer.close();

    expect(screen.queryByText("detail")).toBeNull();
  });

  it("u5_opening_a_drawer_never_resizes_the_terminal_pane", async () => {
    const reports: string[] = [];
    const observed: Element[] = [];
    globalThis.ResizeObserver = class implements ResizeObserver {
      observe(element: Element) {
        observed.push(element);
      }
      unobserve() {}
      disconnect() {}
    };
    const app = createFakeApp();
    app.handlers["rail.tree"] = () => [];
    app.handlers["terminal.list"] = () => [];
    const workspace = await openWorkspace(app, { name: "p", path: "/p" }, () => false);
    render(() => (
      <WorkspaceContext.Provider value={workspace}>
        <Layout
          header="roundup"
          rail={null}
          centre={
            <div
              data-testid="pane"
              ref={(pane) => new ResizeObserver(() => reports.push("resized")).observe(pane)}
            />
          }
          shelf={null}
          overlay={<DrawerHost drawer={workspace.drawer} reducedMotion={() => false} />}
        />
      </WorkspaceContext.Provider>
    ));
    const pane = screen.getByTestId("pane");
    const before = pane.parentElement?.outerHTML;

    workspace.drawer.open(() => <p>detail</p>);

    expect([reports, observed, screen.getByLabelText("drawer").style.position, pane.parentElement?.outerHTML]).toEqual([
      [],
      [pane],
      "absolute",
      before,
    ]);
    expect(pane.closest("[aria-label=drawer]")).toBeNull();
  });

  it("u5_the_last_touch_line_reads_the_newest_touch_in_the_history", () => {
    const history = [touch(USER, "read", at(9, 5)), touch(AGENT, "wrote", at(14, 2)), touch(USER, "read", at(11, 30))];

    expect(lastTouchLine(history, names)).toBe("last  auth-refactor wrote 14:02");
  });

  it("u5_an_item_nobody_touched_has_no_last_touch_line", () => {
    expect(lastTouchLine([], names)).toBeNull();
  });

  it("u5_the_last_touch_component_reads_provenance_history_for_its_item", async () => {
    const app = createFakeApp();
    app.handlers["rail.tree"] = () => [];
    app.handlers["terminal.list"] = () => [];
    app.handlers["provenance.history"] = () => [touch(USER, "wrote", at(8, 7))];
    const workspace = await openWorkspace(app, { name: "p", path: "/p" }, () => false);

    render(() => (
      <WorkspaceContext.Provider value={workspace}>
        <LastTouch item="todo:3" />
      </WorkspaceContext.Provider>
    ));

    expect((await screen.findByText(/^last/)).textContent).toBe("last  you wrote 08:07");
    expect(app.calls.filter((call) => call.method === "provenance.history")).toEqual([
      { method: "provenance.history", params: { item: "todo:3" } },
    ]);
  });
});
