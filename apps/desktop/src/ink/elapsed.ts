const MINUTE_MS = 60_000;

const HOUR_MS = 60 * MINUTE_MS;

const DAY_MS = 24 * HOUR_MS;

export const elapsed = (since: number, now: number): string => {
  const age = now - since;

  if (age < MINUTE_MS) return "just now";

  if (age < HOUR_MS) return `${Math.floor(age / MINUTE_MS)}m`;

  if (age < DAY_MS) return `${Math.floor(age / HOUR_MS)}h`;

  return `${Math.floor(age / DAY_MS)}d`;
};
