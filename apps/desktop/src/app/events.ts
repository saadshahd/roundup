import type { Event as DaemonEvent } from "@contracts/Event";
import type { AppSeam, Unsubscribe } from "./seam";

export type Events = {
  /** Listeners hear every Event from the moment they subscribe, in the Daemon's order. */
  subscribe(listener: (event: DaemonEvent) => void): Unsubscribe;
};

/** Opens the single Daemon subscription and fans it out; resolves once the Daemon is sending, so a caller that fetches next misses nothing. */
export const connectEvents = async (app: AppSeam): Promise<Events> => {
  const listeners = new Set<(event: DaemonEvent) => void>();

  await app.subscribe((event) => {
    for (const listener of listeners) listener(event);
  });

  return {
    subscribe: (listener) => {
      listeners.add(listener);

      return () => listeners.delete(listener);
    },
  };
};
