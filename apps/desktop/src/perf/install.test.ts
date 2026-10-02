import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from "vitest";
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

const paneHeaderStopButton = (): HTMLElement => {
  const pane = document.createElement("div");

  pane.className = "pane";

  const header = document.createElement("div");

  header.className = "pane-header";

  const stop = document.createElement("button");

  header.append(stop);
  pane.append(header);
  document.body.append(pane);

  return stop;
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
  let addEventListenerSpy: MockInstance<Window["addEventListener"]>;

  beforeEach(() => {
    addEventListenerSpy = vi.spyOn(window, "addEventListener");
  });

  afterEach(() => {
    for (const [type, listener, options] of addEventListenerSpy.mock.calls) {
      if (type === "keydown") window.removeEventListener(type, listener, options);
    }

    addEventListenerSpy.mockRestore();
    document.body.replaceChildren();
    delete window.__perf;
    vi.useRealTimers();
  });

  it("u39_without_perf_in_the_url_the_app_passes_through_untouched", async () => {
    const { app, wrapped } = setup("");

    expect(wrapped).toBe(app);
    expect(window.__perf).toBeUndefined();

    await wrapped.subscribe(() => undefined);
    paneKeydownTarget().dispatchEvent(keydown());
    app.emit(outputEvent("t1"));

    expect(window.__perf).toBeUndefined();
    expect(addEventListenerSpy).not.toHaveBeenCalledWith("keydown", expect.anything(), expect.anything());
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

  it("u39_two_keystrokes_before_one_output_both_close_against_it", async () => {
    const { app, wrapped, setTime, runFrame } = setup("?perf");

    await wrapped.subscribe(() => undefined);

    setTime(0);
    paneKeydownTarget().dispatchEvent(keydown());

    setTime(10);
    paneKeydownTarget().dispatchEvent(keydown());

    setTime(20);
    app.emit(outputEvent("t1"));
    runFrame();

    expect(window.__perf?.keystrokeToRender).toEqual([20, 10]);
  });

  it("u39_a_keystroke_typed_after_the_output_arrived_waits_for_the_next_output", async () => {
    const { app, wrapped, setTime, runFrame } = setup("?perf");

    await wrapped.subscribe(() => undefined);

    setTime(0);
    paneKeydownTarget().dispatchEvent(keydown());

    setTime(20);
    app.emit(outputEvent("t1"));

    setTime(22);
    paneKeydownTarget().dispatchEvent(keydown());

    setTime(30);
    runFrame();

    expect(window.__perf?.keystrokeToRender).toEqual([30]);

    setTime(40);
    app.emit(outputEvent("t2"));
    runFrame();

    expect(window.__perf?.keystrokeToRender).toEqual([30, 18]);
  });

  it("u39_a_keydown_outside_the_pane_is_not_stamped", async () => {
    const { app, wrapped, runFrame } = setup("?perf");

    await wrapped.subscribe(() => undefined);

    document.body.dispatchEvent(keydown());
    app.emit(outputEvent("t1"));
    runFrame();

    expect(window.__perf?.keystrokeToRender).toEqual([]);
  });

  it("u39_a_keydown_on_the_pane_header_is_not_stamped", async () => {
    const { app, wrapped, runFrame } = setup("?perf");

    await wrapped.subscribe(() => undefined);

    paneHeaderStopButton().dispatchEvent(keydown());
    app.emit(outputEvent("t1"));
    runFrame();

    expect(window.__perf?.keystrokeToRender).toEqual([]);
  });

  it("u39_a_keydown_whose_target_is_not_an_element_is_not_stamped", async () => {
    const { app, wrapped, runFrame } = setup("?perf");
    const onError = vi.fn();

    window.addEventListener("error", onError);

    await wrapped.subscribe(() => undefined);

    document.dispatchEvent(keydown());
    app.emit(outputEvent("t1"));
    runFrame();

    expect(onError).not.toHaveBeenCalled();
    expect(window.__perf?.keystrokeToRender).toEqual([]);

    window.removeEventListener("error", onError);
  });

  it("u39_capture_stamps_a_keydown_even_if_the_pane_stops_it_from_bubbling", async () => {
    const { app, wrapped, setTime, runFrame } = setup("?perf");

    await wrapped.subscribe(() => undefined);

    const input = paneKeydownTarget();

    input.addEventListener("keydown", (keydownEvent) => keydownEvent.stopPropagation());

    setTime(5);
    input.dispatchEvent(keydown());

    setTime(21);
    app.emit(outputEvent("t1"));
    runFrame();

    expect(window.__perf?.keystrokeToRender).toEqual([16]);
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

  it("u39_an_unanswered_keystroke_is_dropped_and_counted_even_when_nothing_else_happens", async () => {
    vi.useFakeTimers();

    const { wrapped, setTime } = setup("?perf");

    await wrapped.subscribe(() => undefined);

    setTime(0);
    paneKeydownTarget().dispatchEvent(keydown());

    setTime(1000);
    vi.advanceTimersByTime(1000);

    expect(window.__perf?.unanswered).toBe(1);
    expect(window.__perf?.keystrokeToRender).toEqual([]);
  });

  it("u39_p95_reads_through_the_installed_hook", async () => {
    const { app, wrapped, setTime, runFrame } = setup("?perf");

    await wrapped.subscribe(() => undefined);

    for (const elapsed of [10, 20, 30, 40]) {
      setTime(0);
      paneKeydownTarget().dispatchEvent(keydown());

      setTime(elapsed);
      app.emit(outputEvent("t1"));
      runFrame();
    }

    expect(window.__perf?.p95()).toBe(40);
  });
});
