/** `window.__perf`'s shape (U39): the samples a caller reads, never writes. */
export type PerfApi = {
  readonly keystrokeToRender: readonly number[];
  readonly unanswered: number;
  p95(): number | null;
};

export const UNANSWERED_AFTER_MS = 1000;

/** Steady-state cap on `keystrokeToRender`, so a long `?perf` run holds flat memory, like U34's scrollback bound. */
const MAX_SAMPLES = 1000;

/** Nearest-rank percentile; mirrors `crates/perf/src/measure.rs`'s `percentile`. */
const percentile95 = (values: readonly number[]): number | null => {
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.ceil(sorted.length * 0.95) - 1;

  return sorted[index] ?? null;
};

type Stamp = { time: number };

type Tracker = {
  api: PerfApi;
  stamp(now: number): void;
  /** Counts and drops every stamp that has been open for `UNANSWERED_AFTER_MS` as of `now`. */
  dropStale(now: number): void;
  /**
   * Hands every stamp still open at `now` to a closer the caller runs once the frame after the
   * event's handlers has passed, so a keystroke typed after this output arrived is left open for
   * the next one instead of being paired with an output that isn't "after" it.
   */
  outputArrived(now: number): (closedAt: number) => void;
};

export const createKeystrokeToRender = (): Tracker => {
  const pending: Stamp[] = [];
  const keystrokeToRender: number[] = [];
  let unanswered = 0;

  const dropStale = (now: number): void => {
    while (true) {
      const oldest = pending[0];

      if (oldest === undefined || now - oldest.time < UNANSWERED_AFTER_MS) break;

      pending.shift();
      unanswered += 1;
    }
  };

  const record = (elapsed: number): void => {
    keystrokeToRender.push(elapsed);

    if (keystrokeToRender.length > MAX_SAMPLES) keystrokeToRender.shift();
  };

  return {
    api: {
      keystrokeToRender,
      get unanswered() {
        return unanswered;
      },
      p95: () => percentile95(keystrokeToRender),
    },
    stamp: (now) => {
      pending.push({ time: now });
    },
    dropStale,
    outputArrived: (now) => {
      dropStale(now);

      const closing = pending.splice(0, pending.length);

      return (closedAt) => {
        for (const entry of closing) record(closedAt - entry.time);
      };
    },
  };
};
