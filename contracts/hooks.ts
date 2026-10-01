/** Extension Hook types. See docs/extension-interface.md. */

import type { Actor } from "./generated/Actor";
import type { Kind } from "./generated/Kind";
import type { Status } from "./generated/Status";

export type { Actor, Kind, Status };

export type Chip = { label: string; tone: "dim" | "info" };

export type Delivery = "auto" | "ask-first" | "drop";

/** A Hook may add Chips; it can never change Kind. */
export type HookResults = {
  "rail.group": { group: string[] };
  "agent.status": { status: Chip[] };
  "agent.classify": { tags: string[] };
  "bus.route": { deliver: Delivery };
  "tool.call": { deny: string } | { context: string };
};

export type HookName = keyof HookResults;

/** Pass the event on, optionally rewritten for later Hooks. */
export type Next<E> = (event: E) => HookResults[HookName] | undefined;

export type Hook<E, R> = (engine: Engine, event: E, next: Next<E>) => R | undefined;

/** Typed engine handle (`$`). Members are filled in as their modules land. */
export type Engine = { clock: { now: () => number } };
