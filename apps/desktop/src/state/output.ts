import type { OutputEvent } from "@contracts/terminal/OutputEvent";
import type { Events } from "../app/events";
import type { Unsubscribe } from "../app/seam";

/** Base64 characters held per Terminal while nobody listens; the oldest chunks go first. */
export const MAX_HELD_CHARS = 1_048_576;

export type OutputFeed = {
  /** Replays what arrived while nobody listened (in the Daemon's order), then delivers live output. */
  subscribe(listener: (output: OutputEvent) => void): Unsubscribe;
};

/**
 * `terminal.output` is never replayed by the Daemon, and the Pane mounts after the first fetches.
 * This feed listens from the first subscription on and holds output only while no listener is attached.
 */
export const createOutputFeed = (events: Events): OutputFeed => {
  const listeners = new Set<(output: OutputEvent) => void>();
  let held: OutputEvent[] = [];

  const heldChars = (id: string): number =>
    held.reduce((total, chunk) => (chunk.id === id ? total + chunk.data.length : total), 0);

  events.subscribe((event) => {
    if (event.name !== "terminal.output") return;

    if (listeners.size > 0) {
      for (const listener of listeners) listener(event.data);

      return;
    }

    held.push(event.data);

    while (heldChars(event.data.id) > MAX_HELD_CHARS) {
      const oldest = held.findIndex((chunk) => chunk.id === event.data.id);

      held.splice(oldest, 1);
    }
  });

  return {
    subscribe: (listener) => {
      const replay = held;

      held = [];
      listeners.add(listener);

      for (const chunk of replay) listener(chunk);

      return () => listeners.delete(listener);
    },
  };
};
