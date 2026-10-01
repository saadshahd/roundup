/** `window.__perf`'s shape (U39): the samples a caller reads, never writes. */
export type PerfApi = {
  readonly keystrokeToRender: readonly number[];
  readonly unanswered: number;
  p95(): number | null;
};

const UNANSWERED_AFTER_MS = 1000;

/** Steady-state cap on `keystrokeToRender`, so a long `?perf` run holds flat memory, like U34's scrollback bound. */
const MAX_SAMPLES = 1000;

/** Nearest-rank percentile, `values.length > 0`; mirrors `crates/perf/src/measure.rs`'s `percentile`. */
const percentile95 = (values: readonly number[]): number | null => {
  if (values.length === 0) return null;

  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.ceil(sorted.length * 0.95) - 1;
  const value = sorted[index];

  if (value === undefined) throw new Error(`percentile index ${index} out of range for ${sorted.length} values`);

  return value;
};

type Tracker = {
  api: PerfApi;
  /** Opens a stamp for a keystroke typed at `now`. */
  stamp(now: number): void;
  /** Closes the oldest open stamp against a render that landed at `now`. */
  close(now: number): void;
};

/** FIFO of open stamps, oldest first: the next `close` always answers the earliest unclosed `stamp`. */
export const createKeystrokeToRender = (): Tracker => {
  const pending: number[] = [];
  const keystrokeToRender: number[] = [];
  let unanswered = 0;

  const dropStale = (now: number): void => {
    while (pending.length > 0) {
      const oldest = pending[0];

      if (oldest === undefined) throw new Error("pending queue invariant violated: length > 0 with no head");

      if (now - oldest < UNANSWERED_AFTER_MS) break;

      pending.shift();
      unanswered += 1;
    }
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
      dropStale(now);
      pending.push(now);
    },
    close: (now) => {
      dropStale(now);

      const opened = pending.shift();

      if (opened === undefined) return;

      keystrokeToRender.push(now - opened);

      if (keystrokeToRender.length > MAX_SAMPLES) keystrokeToRender.shift();
    },
  };
};
