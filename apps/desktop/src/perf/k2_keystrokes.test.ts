import { describe, expect, it } from "vitest";
import { PATIENCE_MS, createKeystrokeProbe, typeKeys } from "./keystrokes";

const bytes = (text: string) => new TextEncoder().encode(text);

const clock = () => {
  let at = 0;

  return { now: () => at, advance: (ms: number) => void (at += ms) };
};

describe("k2 a sample is keydown to the render that shows its echo", () => {
  it("k2_the_render_after_the_echo_counts_as_the_time_since_the_keydown", () => {
    const time = clock();
    const probe = createKeystrokeProbe(time.now, "a");

    void probe.keydown();
    time.advance(3);
    probe.written(bytes("a"));
    time.advance(9);
    probe.rendered();

    expect(probe.summary(0)).toMatchObject({ n: 1, dropped: 0, p50: 12 });
  });

  it("k2_a_render_before_the_echo_is_not_a_sample", () => {
    const time = clock();
    const probe = createKeystrokeProbe(time.now, "a");

    void probe.keydown();
    time.advance(5);
    probe.rendered();

    expect(() => probe.summary(0)).toThrow("no key was echoed");
  });

  it("k2_output_that_is_not_the_typed_character_is_not_the_echo", () => {
    const time = clock();
    const probe = createKeystrokeProbe(time.now, "a");

    void probe.keydown();
    probe.written(bytes("ready\n"));
    probe.rendered();

    expect(() => probe.summary(0)).toThrow("no key was echoed");
  });

  it("k2_a_key_with_no_rendered_echo_in_time_is_dropped_and_counted", async () => {
    const time = clock();
    const probe = createKeystrokeProbe(time.now, "a");
    const sleeps: number[] = [];

    await typeKeys(
      probe,
      { press: () => {}, sleep: async (ms) => void sleeps.push(ms), random: () => 0.5 },
      1,
    );

    expect(sleeps).toEqual([PATIENCE_MS, 30]);

    void probe.keydown();
    probe.written(bytes("a"));
    time.advance(4);
    probe.rendered();

    expect(probe.summary(0)).toMatchObject({ n: 1, dropped: 1, p50: 4 });
  });

  it("k2_the_first_samples_are_skipped_and_the_rest_summarised_by_nearest_rank", () => {
    const time = clock();
    const probe = createKeystrokeProbe(time.now, "a");

    for (const wait of [100, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]) {
      void probe.keydown();
      probe.written(bytes("a"));
      time.advance(wait);
      probe.rendered();
    }

    expect(probe.summary(1)).toEqual({ n: 20, dropped: 0, p50: 10, p95: 19, p99: 20, max: 20 });
  });
});
