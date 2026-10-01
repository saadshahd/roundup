import { createSignal, onCleanup } from "solid-js";
import type { Accessor } from "solid-js";

const TICK_MS = 15_000;

/** Milliseconds since the Unix epoch, refreshed often enough that `4m` turns into `5m` on time. */
export const createClock = (): Accessor<number> => {
  const [now, setNow] = createSignal(Date.now());
  const timer = setInterval(() => setNow(Date.now()), TICK_MS);

  onCleanup(() => clearInterval(timer));

  return now;
};
