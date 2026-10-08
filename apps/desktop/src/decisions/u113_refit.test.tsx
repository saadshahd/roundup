import { waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { callsTo, card, cleared, decision, mountApp, opened, select } from "./decisionsFixture";

let watchers: (() => void)[] = [];

class FakeResizeObserver {
  constructor(callback: () => void) {
    watchers.push(callback);
  }

  observe() {}

  unobserve() {}

  disconnect() {}
}

const layoutChanged = () => {
  for (const watcher of watchers) watcher();
};

beforeEach(() => {
  watchers = [];
  vi.stubGlobal("ResizeObserver", FakeResizeObserver);
  vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] });
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("u113 the Terminal refits around the Card", () => {
  it("u113_card_appearance_replacement_and_removal_refit_once_per_frame_and_keep_the_output", async () => {
    const { app, emulators } = await mountApp([]);

    select("alpha");

    const emulator = emulators.made.get("t-a")!;
    const written = [...emulator.written];
    const baseline = callsTo(app, "terminal.resize").length;

    emulator.size = { cols: 100, rows: 20 };
    app.emit(opened(decision("d1", "a")));
    await waitFor(() => expect(card()).not.toBeNull());
    layoutChanged();
    layoutChanged();
    layoutChanged();
    vi.advanceTimersToNextFrame();

    expect(callsTo(app, "terminal.resize").slice(baseline)).toEqual([{ id: "t-a", cols: 100, rows: 20 }]);

    emulator.size = { cols: 100, rows: 12 };
    app.emit(opened(decision("d2", "a", { tool: "Write", opened_at: 2 })));
    app.emit(cleared("d1", "replaced"));
    layoutChanged();
    vi.advanceTimersToNextFrame();

    expect(callsTo(app, "terminal.resize").slice(baseline).at(-1)).toEqual({ id: "t-a", cols: 100, rows: 12 });

    emulator.size = { cols: 100, rows: 30 };
    app.emit(cleared("d2"));
    await waitFor(() => expect(card()).toBeNull());
    layoutChanged();
    vi.advanceTimersToNextFrame();

    expect(callsTo(app, "terminal.resize").slice(baseline).at(-1)).toEqual({ id: "t-a", cols: 100, rows: 30 });
    expect(callsTo(app, "terminal.resize").slice(baseline)).toHaveLength(3);
    expect(emulator.written).toEqual(written);
    expect(emulator.scrollsToBottom).toBe(0);
    expect(emulator.disposed).toBe(false);
  });
});
