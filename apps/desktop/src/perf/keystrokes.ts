/** How long a key may take to come back as a rendered echo before its sample is dropped. */
export const PATIENCE_MS = 500;

/** Keys typed and thrown away first, so the first renders (font load, glyph atlas) do not count. */
export const WARMUP_KEYS = 100;

export type Summary = { n: number; dropped: number; p50: number; p95: number; p99: number; max: number };

/** Nearest rank, the same rule as `crates/perf`. */
const percentile = (sorted: number[], share: number): number => {
  const value = sorted[Math.max(Math.ceil(share * sorted.length), 1) - 1];

  if (value === undefined) throw new Error("no key was echoed and rendered after the warm-up");

  return value;
};

/** Keydown to the first render that shows its echo. A render before the echo reached the screen's buffer is the previous frame's and does not count. */
export const createKeystrokeProbe = (now: () => number, typed: string) => {
  const latencies: { key: number; ms: number }[] = [];
  const droppedKeys: number[] = [];
  let pending: { key: number; at: number; echoed: boolean; counted: () => void } | null = null;
  let keys = 0;

  return {
    /** Resolves when the key's echo has been rendered, or at `abandon`. */
    keydown: (): Promise<void> =>
      new Promise((resolve) => {
        if (pending) droppedKeys.push(pending.key);

        pending = { key: keys, at: now(), echoed: false, counted: resolve };
        keys += 1;
      }),
    /** Called with each chunk the emulator has parsed. Only the typed character itself is the echo; anything else is the program's own output. */
    written: (bytes: Uint8Array) => {
      if (pending && new TextDecoder().decode(bytes) === typed) pending.echoed = true;
    },
    rendered: () => {
      if (!pending?.echoed) return;

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
    /** `skip` is the number of leading keys typed, whether or not they came back: warm-up drops stay out of the count. */
    summary: (skip: number): Summary => {
      const sorted = latencies
        .filter(({ key }) => key >= skip)
        .map(({ ms }) => ms)
        .sort((a, b) => a - b);

      return {
        n: sorted.length,
        dropped: droppedKeys.filter((key) => key >= skip).length,
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
};

/** Types `count` keys one at a time, 20 to 40 ms apart so the key's phase against the display frame is not constant. */
export const typeKeys = async (probe: KeystrokeProbe, surroundings: Surroundings, count: number): Promise<void> => {
  for (let key = 0; key < count; key += 1) {
    const echoed = probe.keydown();

    surroundings.press();
    await Promise.race([echoed, surroundings.sleep(PATIENCE_MS)]);
    probe.abandon();
    await surroundings.sleep(20 + surroundings.random() * 20);
  }
};
