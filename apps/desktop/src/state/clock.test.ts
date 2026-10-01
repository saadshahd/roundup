import { createRoot } from "solid-js";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createNow, NOW_TICK_MS } from "./clock";

beforeEach(() => vi.useFakeTimers());

afterEach(() => vi.useRealTimers());

describe("u4 the reactive clock", () => {
  it("u4_now_starts_at_the_injected_clock", () => {
    createRoot((dispose) => {
      expect(createNow(() => 1000)()).toBe(1000);
      dispose();
    });
  });

  it("u4_now_follows_the_injected_clock_each_tick", () => {
    let time = 1000;

    createRoot((dispose) => {
      const now = createNow(() => time);
      time = 61_000;
      vi.advanceTimersByTime(NOW_TICK_MS);

      expect(now()).toBe(61_000);
      dispose();
    });
  });

  it("u4_now_does_not_move_between_ticks", () => {
    let time = 1000;

    createRoot((dispose) => {
      const now = createNow(() => time);
      time = 61_000;
      vi.advanceTimersByTime(NOW_TICK_MS - 1);

      expect(now()).toBe(1000);
      dispose();
    });
  });

  it("u4_one_timer_serves_the_app_and_unmounting_clears_it", () => {
    const counts: number[] = [];

    createRoot((dispose) => {
      createNow(() => 0);
      counts.push(vi.getTimerCount());
      dispose();
    });
    counts.push(vi.getTimerCount());

    expect(counts).toEqual([1, 0]);
  });
});
