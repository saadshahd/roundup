import type { AppSeam } from "../app/seam";
import { createKeystrokeToRender } from "./keystrokeToRender";
import type { PerfApi } from "./keystrokeToRender";

declare global {
  interface Window {
    __perf?: PerfApi;
  }
}

const isInPane = (target: EventTarget | null): boolean =>
  target instanceof Element && target.closest(".pane") !== null;

/**
 * U39: with `?perf` in `search`, wires a keystroke-to-render hook onto the App seam and publishes
 * `window.__perf`; without it, `app` passes through with no listener installed and no global set.
 */
export const installPerfHook = (
  app: AppSeam,
  search: string,
  now: () => number,
  frame: (callback: () => void) => void,
): AppSeam => {
  if (!new URLSearchParams(search).has("perf")) return app;

  const tracker = createKeystrokeToRender();

  window.__perf = tracker.api;
  window.addEventListener(
    "keydown",
    (keydownEvent) => {
      if (isInPane(keydownEvent.target)) tracker.stamp(now());
    },
    { capture: true },
  );

  return {
    ...app,
    subscribe: async (onEvent) => {
      await app.subscribe((daemonEvent) => {
        onEvent(daemonEvent);

        if (daemonEvent.name === "terminal.output") frame(() => tracker.close(now()));
      });
    },
  };
};
