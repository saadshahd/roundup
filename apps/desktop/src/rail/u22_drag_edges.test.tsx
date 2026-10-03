import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RailNode } from "@contracts/agent/RailNode";
import { dragFrom, pointerAt, release, stubLayout } from "./dragFixture";
import { mountRail, railCallsTo } from "./railFixture";
import { agent, group } from "../testing/nodes";

/** a, g { x }, b: the Rail is 200 px wide and 400 px high (`stubLayout`). */
const TREE: RailNode[] = [
  agent("a", "idle", "i", { order: 0 }),
  group("g", { order: 1 }),
  agent("x", "idle", "i", { parent: "g", order: 0 }),
  agent("b", "idle", "i", { order: 2 }),
];

/** U22: within this many pixels of the Rail's top or bottom edge a drag scrolls the Rail. */
const EDGE_PX = 32;

/** U22: pixels scrolled per animation frame while the pointer is in that zone. */
const STEP_PX = 12;

const FRAME_MS = 16;

beforeEach(() => {
  stubLayout();
  vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] });
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

const dropLine = () => document.querySelector<HTMLElement>(".drop-line");

const shifted = () => screen.getAllByRole("treeitem").filter((row) => row.dataset.shift !== "0");

/** jsdom does not scroll: the Rail's scroll position is a plain number a test sets and reads. */
const scrollableRail = (scrollTop: number, scrollHeight = 1000, clientHeight = 400) => {
  const rail = document.querySelector<HTMLElement>(".rail-tree")!;
  let top = scrollTop;

  Object.defineProperty(rail, "scrollTop", { get: () => top, set: (value: number) => void (top = value), configurable: true });
  Object.defineProperty(rail, "scrollHeight", { value: scrollHeight, configurable: true });
  Object.defineProperty(rail, "clientHeight", { value: clientHeight, configurable: true });

  return rail;
};

const frames = (count: number) => vi.advanceTimersByTime(count * FRAME_MS);

describe("u22 a release outside the Rail", () => {
  it("u22_a_release_beyond_the_rails_right_edge_calls_nothing", async () => {
    const mounted = await mountRail(TREE);
    mounted.app.handlers["rail.move"] = () => null;

    dragFrom("b", { clientX: 300, clientY: 100 });
    release({ clientX: 300, clientY: 100 });

    await waitFor(() => expect(railCallsTo(mounted.app, "rail.move")).toEqual([]));
  });

  it("u22_a_release_below_the_rails_bottom_edge_calls_nothing", async () => {
    const mounted = await mountRail(TREE);
    mounted.app.handlers["rail.move"] = () => null;

    dragFrom("a", { clientX: 100, clientY: 450 });
    release({ clientX: 100, clientY: 450 });

    await waitFor(() => expect(railCallsTo(mounted.app, "rail.move")).toEqual([]));
  });

  it("u22_no_drop_line_shows_and_no_row_makes_room_while_the_pointer_is_outside_the_rail", async () => {
    await mountRail(TREE);

    dragFrom("b", pointerAt(1, 0));
    expect(dropLine()).not.toBeNull();

    fireEvent.pointerMove(window, { clientX: 300, clientY: 20 });

    expect(dropLine()).toBeNull();
    expect(shifted()).toEqual([]);
  });

  it("u22_coming_back_into_the_rail_shows_the_drop_line_and_a_release_drops", async () => {
    const mounted = await mountRail(TREE);
    mounted.app.handlers["rail.move"] = () => null;

    dragFrom("b", { clientX: 300, clientY: 20 });
    fireEvent.pointerMove(window, pointerAt(0, 0));
    expect(dropLine()).not.toBeNull();
    release(pointerAt(0, 0));

    await waitFor(() => expect(railCallsTo(mounted.app, "rail.move")).toEqual([{ id: "b", parent: null, index: 0 }]));
  });

  it("u22_leaving_the_rail_above_or_to_the_left_after_a_drop_line_showed_calls_nothing", async () => {
    const mounted = await mountRail(TREE);
    mounted.app.handlers["rail.move"] = () => null;

    dragFrom("b", pointerAt(0, 0));
    fireEvent.pointerMove(window, { clientX: -10, clientY: 20 });
    expect(dropLine()).toBeNull();
    fireEvent.pointerMove(window, { clientX: 100, clientY: -10 });
    release({ clientX: 100, clientY: -10 });

    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(railCallsTo(mounted.app, "rail.move")).toEqual([]);
  });
});

describe("u22 a drag near the Rail's top or bottom edge scrolls it", () => {
  it("u22_a_pointer_inside_the_bottom_edge_zone_scrolls_the_rail_down_each_frame", async () => {
    await mountRail(TREE);
    const rail = scrollableRail(0);

    dragFrom("b", { clientX: 100, clientY: 400 - EDGE_PX + 1 });
    frames(3);

    expect(rail.scrollTop).toBe(3 * STEP_PX);
  });

  it("u22_a_pointer_inside_the_top_edge_zone_scrolls_the_rail_up_each_frame", async () => {
    await mountRail(TREE);
    const rail = scrollableRail(100);

    dragFrom("b", { clientX: 100, clientY: EDGE_PX - 1 });
    frames(2);

    expect(rail.scrollTop).toBe(100 - 2 * STEP_PX);
  });

  it("u22_a_pointer_outside_the_zones_scrolls_nothing", async () => {
    await mountRail(TREE);
    const rail = scrollableRail(100);

    dragFrom("b", { clientX: 100, clientY: 200 });
    frames(5);

    expect(rail.scrollTop).toBe(100);
  });

  it("u22_the_scroll_stops_at_the_end_of_the_rail", async () => {
    await mountRail(TREE);
    const rail = scrollableRail(590, 1000, 400);

    dragFrom("b", { clientX: 100, clientY: 395 });
    frames(10);

    expect(rail.scrollTop).toBe(600);
  });

  it("u22_a_release_stops_the_scroll", async () => {
    await mountRail(TREE);
    const rail = scrollableRail(0);

    dragFrom("b", { clientX: 100, clientY: 395 });
    frames(1);
    release({ clientX: 100, clientY: 395 });
    frames(5);

    expect(rail.scrollTop).toBe(STEP_PX);
  });

  it("u22_escape_stops_the_scroll", async () => {
    await mountRail(TREE);
    const rail = scrollableRail(0);

    dragFrom("b", { clientX: 100, clientY: 395 });
    frames(1);
    fireEvent.keyDown(window, { key: "Escape" });
    frames(5);

    expect(rail.scrollTop).toBe(STEP_PX);
  });

  it("u22_moving_out_of_the_zone_stops_the_scroll", async () => {
    await mountRail(TREE);
    const rail = scrollableRail(0);

    dragFrom("b", { clientX: 100, clientY: 395 });
    frames(1);
    fireEvent.pointerMove(window, { clientX: 100, clientY: 200 });
    frames(5);

    expect(rail.scrollTop).toBe(STEP_PX);
  });

  it("u22_a_pointer_just_outside_either_zone_scrolls_nothing", async () => {
    await mountRail(TREE);
    const rail = scrollableRail(100);

    dragFrom("b", { clientX: 100, clientY: 400 - EDGE_PX - 1 });
    frames(3);
    fireEvent.pointerMove(window, { clientX: 100, clientY: EDGE_PX + 1 });
    frames(3);

    expect(rail.scrollTop).toBe(100);
  });

  it("u22_the_drop_line_follows_the_pointer_while_the_rail_scrolls", async () => {
    const mounted = await mountRail(Array.from({ length: 30 }, (_, n) => agent(`r${n}`, "idle", "i", { order: n })));
    mounted.app.handlers["rail.move"] = () => null;
    const rail = scrollableRail(0, 600, 400);

    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      if (this.getAttribute("role") === "treeitem") return new DOMRect(0, screen.getAllByRole("treeitem").indexOf(this) * 20 - rail.scrollTop, 200, 20);

      if (this.classList.contains("rail-tree")) return new DOMRect(0, 0, 200, 400);

      return new DOMRect(0, 0, 20, 20);
    });

    dragFrom("r0", { clientX: 0, clientY: 380 });
    frames(5);
    release({ clientX: 0, clientY: 380 });

    await waitFor(() => expect(railCallsTo(mounted.app, "rail.move")).toEqual([{ id: "r0", parent: null, index: 21 }]));
  });
});
