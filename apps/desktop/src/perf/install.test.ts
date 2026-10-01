import { afterEach, describe, expect, it } from "vitest";
import type { Event as DaemonEvent } from "@contracts/Event";
import { createFakeApp } from "../testing/fakeApp";
import { event } from "../testing/nodes";
import { installPerfHook } from "./install";

const paneKeydownTarget = (): HTMLElement => {
  const pane = document.createElement("div");

  pane.className = "pane";

  const screen = document.createElement("div");

  screen.className = "pane-screen";

  const input = document.createElement("textarea");

  screen.append(input);
  pane.append(screen);
  document.body.append(pane);

  return input;
};

const outputEvent = (id: string): DaemonEvent => event({ name: "terminal.output", data: { id, data: "" } });

const keydown = (): KeyboardEvent => new KeyboardEvent("keydown", { bubbles: true });

const setup = (search: string) => {
  const app = createFakeApp();
  let currentTime = 0;
  let pendingFrame: (() => void) | null = null;
  let frameCalls = 0;

  const wrapped = installPerfHook(
    app,
    search,
    () => currentTime,
    (callback) => {
      frameCalls += 1;
      pendingFrame = callback;
    },
  );

  return {
    app,
    wrapped,
    setTime: (time: number) => {
      currentTime = time;
    },
    runFrame: () => {
      const callback = pendingFrame;

      pendingFrame = null;
      callback?.();
    },
    frameCalls: () => frameCalls,
  };
};

describe("u39 the ?perf keystroke-to-render hook", () => {
  afterEach(() => {
    document.body.replaceChildren();
    delete window.__perf;
  });

  it("u39_without_perf_in_the_url_the_app_passes_through_untouched", async () => {
    const { app, wrapped } = setup("");

    expect(wrapped).toBe(app);
    expect(window.__perf).toBeUndefined();

    await wrapped.subscribe(() => undefined);
    paneKeydownTarget().dispatchEvent(keydown());
    app.emit(outputEvent("t1"));

    expect(window.__perf).toBeUndefined();
  });

  it("u39_perf_is_recognised_alongside_other_query_params", () => {
    setup("?seed=tree-40&perf");

    expect(window.__perf).toBeDefined();
  });

  it("u39_a_pane_keystroke_closes_once_the_next_outputs_handlers_and_a_frame_have_run", async () => {
    const { app, wrapped, setTime, runFrame } = setup("?perf");
    const received: DaemonEvent[] = [];

    await wrapped.subscribe((received_event) => received.push(received_event));

    setTime(5);
    paneKeydownTarget().dispatchEvent(keydown());

    setTime(21);
    app.emit(outputEvent("t1"));

    expect(received).toHaveLength(1);
    expect(window.__perf?.keystrokeToRender).toEqual([]);

    runFrame();

    expect(window.__perf?.keystrokeToRender).toEqual([16]);
  });

  it("u39_a_keydown_outside_the_pane_is_not_stamped", async () => {
    const { app, wrapped, runFrame } = setup("?perf");

    await wrapped.subscribe(() => undefined);

    document.body.dispatchEvent(keydown());
    app.emit(outputEvent("t1"));
    runFrame();

    expect(window.__perf?.keystrokeToRender).toEqual([]);
  });

  it("u39_a_non_output_event_schedules_no_frame", async () => {
    const { app, wrapped, frameCalls } = setup("?perf");

    await wrapped.subscribe(() => undefined);
    app.emit(event({ name: "rail.changed" }));

    expect(frameCalls()).toBe(0);
  });

  it("u39_forwards_every_event_to_the_real_subscriber_unchanged", async () => {
    const { app, wrapped } = setup("?perf");
    const received: DaemonEvent[] = [];
    const railChanged = event({ name: "rail.changed" });

    await wrapped.subscribe((received_event) => received.push(received_event));
    app.emit(railChanged);

    expect(received).toEqual([railChanged]);
  });

  it("u39_an_unanswered_keystroke_is_counted_through_the_installed_hook", async () => {
    const { app, wrapped, setTime, runFrame } = setup("?perf");

    await wrapped.subscribe(() => undefined);

    setTime(0);
    paneKeydownTarget().dispatchEvent(keydown());

    setTime(1000);
    paneKeydownTarget().dispatchEvent(keydown());

    setTime(1010);
    app.emit(outputEvent("t1"));
    runFrame();

    expect(window.__perf?.unanswered).toBe(1);
    expect(window.__perf?.keystrokeToRender).toEqual([10]);
  });
});
