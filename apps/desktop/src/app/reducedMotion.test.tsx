import { cleanup, render, screen } from "@solidjs/testing-library";
import { createRoot } from "solid-js";
import { afterEach, describe, expect, it } from "vitest";
import { createDrawer } from "../drawer/drawer";
import { DrawerHost } from "../drawer/DrawerHost";
import { createReducedMotion } from "./reducedMotion";
import type { MediaQuery } from "./reducedMotion";

const REDUCE = "(prefers-reduced-motion: reduce)";

/** A `matchMedia` that reports `matches` for the reduced-motion query only. */
const matchMediaWith = (matches: boolean) => (query: string): MediaQuery => ({
  matches: query === REDUCE && matches,
  addEventListener: () => {},
  removeEventListener: () => {},
});

afterEach(cleanup);

describe("u5 reduced motion", () => {
  it("u5_the_app_reads_prefers_reduced_motion_from_the_webview", () => {
    createRoot((dispose) => {
      expect([createReducedMotion(matchMediaWith(true))(), createReducedMotion(matchMediaWith(false))()]).toEqual([
        true,
        false,
      ]);
      dispose();
    });
  });

  it("u5_a_drawer_is_instant_when_the_webview_prefers_reduced_motion", () => {
    const drawer = createDrawer();
    const reduced = createRoot(() => createReducedMotion(matchMediaWith(true)));
    render(() => <DrawerHost drawer={drawer} reducedMotion={reduced} />);

    drawer.open(() => <p>detail</p>);

    expect(screen.getByLabelText("drawer").style.transition).toBe("none");
  });

  it("u5_a_change_of_the_preference_is_followed", () => {
    let notify: (change: { matches: boolean }) => void = () => {};

    const query: MediaQuery = {
      matches: false,
      addEventListener: (_type, listener) => {
        notify = listener;
      },
      removeEventListener: () => {},
    };

    createRoot((dispose) => {
      const reduced = createReducedMotion(() => query);
      notify({ matches: true });

      expect(reduced()).toBe(true);
      dispose();
    });
  });
});
