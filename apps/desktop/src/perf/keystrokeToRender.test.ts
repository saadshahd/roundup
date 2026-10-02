import { describe, expect, it } from "vitest";
import { createKeystrokeToRender } from "./keystrokeToRender";

describe("u39 keystroke-to-render tracking", () => {
  it("u39_p95_is_null_with_no_samples", () => {
    const tracker = createKeystrokeToRender();

    expect(tracker.api.p95()).toBeNull();
  });

  it("u39_every_keystroke_pending_before_an_output_closes_against_it", () => {
    const tracker = createKeystrokeToRender();

    tracker.stamp(0);
    tracker.stamp(10);
    tracker.outputArrived(20)(20);

    expect(tracker.api.keystrokeToRender).toEqual([20, 10]);
  });

  it("u39_a_keystroke_stamped_after_the_output_arrived_waits_for_the_next_one", () => {
    const tracker = createKeystrokeToRender();

    tracker.stamp(0);
    const settle = tracker.outputArrived(20);

    tracker.stamp(22);
    settle(30);

    expect(tracker.api.keystrokeToRender).toEqual([30]);

    tracker.outputArrived(40)(40);

    expect(tracker.api.keystrokeToRender).toEqual([30, 18]);
  });

  it("u39_an_output_with_no_pending_keystroke_closes_nothing", () => {
    const tracker = createKeystrokeToRender();

    tracker.outputArrived(100)(100);

    expect(tracker.api.keystrokeToRender).toEqual([]);
  });

  it("u39_an_idle_unanswered_keystroke_past_one_second_is_dropped_and_counted", () => {
    const tracker = createKeystrokeToRender();

    tracker.stamp(0);
    tracker.dropStale(1000);

    expect(tracker.api.unanswered).toBe(1);
    expect(tracker.api.keystrokeToRender).toEqual([]);
  });

  it("u39_a_keystroke_answered_just_under_one_second_is_not_dropped", () => {
    const tracker = createKeystrokeToRender();

    tracker.stamp(0);
    tracker.outputArrived(999)(999);

    expect(tracker.api.unanswered).toBe(0);
    expect(tracker.api.keystrokeToRender).toEqual([999]);
  });

  it("u39_a_stale_keystroke_at_output_arrival_is_counted_not_closed", () => {
    const tracker = createKeystrokeToRender();

    tracker.stamp(0);
    tracker.outputArrived(1000)(1000);

    expect(tracker.api.unanswered).toBe(1);
    expect(tracker.api.keystrokeToRender).toEqual([]);
  });

  it("u39_p95_is_the_nearest_rank_95th_percentile", () => {
    const tracker = createKeystrokeToRender();

    for (let elapsed = 1; elapsed <= 20; elapsed += 1) {
      tracker.stamp(0);
      tracker.outputArrived(elapsed)(elapsed);
    }

    expect(tracker.api.p95()).toBe(19);
  });

  it("u39_p95_sorts_samples_before_ranking", () => {
    const tracker = createKeystrokeToRender();

    for (const elapsed of [50, 10, 30, 20, 40]) {
      tracker.stamp(0);
      tracker.outputArrived(elapsed)(elapsed);
    }

    expect(tracker.api.keystrokeToRender).toEqual([50, 10, 30, 20, 40]);
    expect(tracker.api.p95()).toBe(50);
  });

  it("u39_keystroke_to_render_keeps_only_the_most_recent_samples", () => {
    const tracker = createKeystrokeToRender();

    for (let i = 0; i < 1001; i += 1) {
      tracker.stamp(i);
      tracker.outputArrived(i + 1)(i + 1);
    }

    expect(tracker.api.keystrokeToRender).toHaveLength(1000);
    expect(tracker.api.keystrokeToRender.at(-1)).toBe(1);
  });
});
