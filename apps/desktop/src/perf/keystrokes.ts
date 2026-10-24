/** How long a key may take to come back as a rendered echo before its sample is dropped. */
export const PATIENCE_MS = 500;

/** Keys typed and thrown away first, so the first renders (font load, glyph atlas) do not count. */
export const WARMUP_KEYS = 100;

export type Summary = { n: number; dropped: number } & (
  | { p50: number; p95: number; p99: number; max: number }
  | { error: string; p50: null; p95: null; p99: null; max: null }
);

/** Nearest rank, the same rule as `crates/perf`. */
const percentile = (sorted: number[], share: number): number => {
  const value = sorted[Math.max(Math.ceil(share * sorted.length), 1) - 1];

  if (value === undefined)
    throw new Error("no key was echoed and rendered after the warm-up");

  return value;
};

/** The ordered boundaries of one key, all read on the webview's clock so no cross-process subtraction is needed. */
export const BOUNDARIES = ["keydown", "input", "encoded", "dispatched", "replied", "arrived", "parsed", "rendered"] as const;

type Boundary = (typeof BOUNDARIES)[number];

type Marks = Partial<Record<Boundary, number>>;

export type HopSummary = { n: number; p50: number | null; p95: number | null };

type Pending = { key: number; at: number; echoed: boolean; counted: () => void; marks: Marks };

/** Keydown to the target Terminal's first render that shows its echo. Other Terminals and renders before its echo reaches the buffer cannot finish a sample. */
export const createKeystrokeProbe = (now: () => number, typed: string, target: () => string | null) => {
  const latencies: { key: number; ms: number }[] = [];
  const finished: { key: number; marks: Marks }[] = [];
  let dispatched: Marks | null = null;
  const droppedKeys: number[] = [];
  let pending: Pending | null = null;
  let keys = 0;
  let unsent = 0;

  return {
    /** Resolves when the key's echo has been rendered, or at `abandon`. */
    keydown: (): Promise<void> =>
      new Promise((resolve) => {
        if (pending) droppedKeys.push(pending.key);

        const at = now();

        pending = { key: keys, at, echoed: false, counted: resolve, marks: { keydown: at } };
        keys += 1;
      }),
    /** Called with each chunk the emulator has parsed. Only the typed character itself is the echo; anything else is the program's own output. */
    written: (bytes: Uint8Array, id: string) => {
      if (id !== target()) return;

      if (pending && new TextDecoder().decode(bytes) === typed) {
        pending.echoed = true;
        pending.marks.parsed ??= now();
      }
    },
    /** xterm's `onData` for the pending key. */
    input: (id: string) => {
      if (id !== target()) return;

      if (pending) pending.marks.input ??= now();
    },
    /** The input queue's current batch has finished base64 encoding; includes any wait for the previous reply. */
    encoded: (id: string) => {
      if (id !== target()) return;

      if (pending?.marks.input !== undefined) pending.marks.encoded ??= now();
    },
    /** `terminal.write` called and, later, answered. */
    dispatched: (id: string) => {
      if (id !== target()) return;

      if (pending?.marks.input === undefined || pending.marks.dispatched !== undefined) return;

      pending.marks.dispatched = now();
      dispatched = pending.marks;
    },
    replied: (id: string) => {
      if (id !== target()) return;

      if (dispatched && dispatched.replied === undefined) dispatched.replied = now();

      dispatched = null;
    },
    /** The output event carrying the typed character reached the webview, before `emulator.write`. */
    arrived: (bytes: Uint8Array, id: string) => {
      if (id !== target()) return;

      if (pending && new TextDecoder().decode(bytes) === typed) pending.marks.arrived ??= now();
    },
    rendered: (id: string) => {
      if (id !== target()) return;

      if (!pending?.echoed) return;

      pending.marks.rendered = now();
      finished.push({ key: pending.key, marks: pending.marks });
      latencies.push({ key: pending.key, ms: now() - pending.at });
      pending.counted();
      pending = null;
    },
    abandon: () => {
      if (!pending) return;

      droppedKeys.push(pending.key);
      pending.counted();
      pending = null;
    },
    /** p50/p95 of each hop over the measured keys that carry both of its boundaries. */
    hops: (skip: number) => {
      const sample = (from: Boundary, to: Boundary): HopSummary => {
        const spans = finished
          .flatMap(({ key, marks }) => {
            const a = marks[from];
            const b = marks[to];

            return key >= skip && a !== undefined && b !== undefined ? [b - a] : [];
          })
          .sort((x, y) => x - y);

        return spans.length === 0
          ? { n: 0, p50: null, p95: null }
          : { n: spans.length, p50: percentile(spans, 0.5), p95: percentile(spans, 0.95) };
      };

      return {
        keydown_to_input: sample("keydown", "input"),
        input_to_dispatch: sample("input", "dispatched"),
        input_to_encoded: sample("input", "encoded"),
        encoded_to_dispatch: sample("encoded", "dispatched"),
        dispatch_to_echo_event: sample("dispatched", "arrived"),
        echo_event_to_parsed: sample("arrived", "parsed"),
        parsed_to_frame: sample("parsed", "rendered"),
        write_reply: sample("dispatched", "replied"),
      };
    },
    /** Keys the run's time ran out before it typed: dropped, never skipped. */
    unsent: (count: number) => {
      unsent += count;
    },
    /** Warm-up latency samples are excluded, but every dropped key fails R21. An empty sample retains counts alongside an error and null percentiles. */
    summary: (skip: number): Summary => {
      const sorted = latencies
        .flatMap(({ key, ms }) => (key >= skip ? [ms] : []))
        .sort((a, b) => a - b);

      const dropped = droppedKeys.length + unsent;

      if (sorted.length === 0) return {
        error: "no key was echoed and rendered after the warm-up",
        n: 0, dropped, p50: null, p95: null, p99: null, max: null,
      };

      return {
        n: sorted.length,
        dropped,
        p50: percentile(sorted, 0.5),
        p95: percentile(sorted, 0.95),
        p99: percentile(sorted, 0.99),
        max: percentile(sorted, 1),
      };
    },
  };
};

export type KeystrokeProbe = ReturnType<typeof createKeystrokeProbe>;

/** What the driver needs from its surroundings; the browser run supplies the DOM, tests supply a fake clock. */
export type Surroundings = {
  press(): void;
  sleep(ms: number): Promise<void>;
  random(): number;
  /** True once the run's time is spent: the keys not yet typed are counted as dropped and the run reports. */
  expired(): boolean;
};

/** Types `count` keys one at a time, 20 to 40 ms apart so the key's phase against the display frame is not constant. Stops when time is spent, so a run of dropped keys still ends in a report. */
export const typeKeys = async (
  probe: KeystrokeProbe,
  surroundings: Surroundings,
  count: number,
): Promise<void> => {
  for (let key = 0; key < count; key += 1) {
    if (surroundings.expired()) {
      probe.unsent(count - key);

      return;
    }

    const echoed = probe.keydown();

    surroundings.press();
    await Promise.race([echoed, surroundings.sleep(PATIENCE_MS)]);
    probe.abandon();
    await surroundings.sleep(20 + surroundings.random() * 20);
  }
};
