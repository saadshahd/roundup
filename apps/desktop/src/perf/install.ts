import type { AppSeam } from "../app/seam";
import { createKeystrokeToRender, UNANSWERED_AFTER_MS } from "./keystrokeToRender";
import type { PerfApi } from "./keystrokeToRender";

declare global {
  interface Window {
    __perf?: PerfApi;
  }
}

const isInPaneScreen = (target: EventTarget | null): boolean =>
  target instanceof Element && target.closest(".pane-screen") !== null;

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
      if (!isInPaneScreen(keydownEvent.target)) return;

      tracker.stamp(now());
      // The App seam raises no event for an output that never comes; this is the only thing that notices.
      setTimeout(() => tracker.dropStale(now()), UNANSWERED_AFTER_MS);
    },
    { capture: true },
  );

  return {
    ...app,
    subscribe: async (onEvent) => {
      await app.subscribe((daemonEvent) => {
        onEvent(daemonEvent);

        if (daemonEvent.name === "terminal.output") {
          const settle = tracker.outputArrived(now());

          frame(() => settle(now()));
        }
      });
    },
  };
};
