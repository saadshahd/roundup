import type { RailNode } from "@contracts/agent/RailNode";
import { exitText } from "../ink/exitText";
import type { ExitState } from "../state/rail";

/** Why a Room's Door has no running program. `stopped` means launched before and not running now, however it ended. */
export type DoorState =
  | { kind: "starting" }
  | { kind: "failed" }
  | { kind: "never-started" }
  | { kind: "stopped"; exit: ExitState };

/** `null` while the Door's program runs; `exit` is `rail.exitOf` for the Room, `pending` and `failed` are the Rail's record of a `rail.startDoor` call. */
export const doorStateOf = (room: RailNode, exit: ExitState | null, pending: boolean, failed: boolean): DoorState | null => {
  if (exit === null) return null;

  if (pending) return { kind: "starting" };

  if (failed) return { kind: "failed" };

  return room.attempt === null ? { kind: "never-started" } : { kind: "stopped", exit };
};

export const doorStateText = (state: DoorState): string => {
  switch (state.kind) {
    case "starting":
      return "starting Door…";
    case "failed":
      return "Door did not start";
    case "never-started":
      return "Door not started";
    case "stopped":
      return state.exit.kind === "unknown" ? "Door stopped" : `Door stopped, ${exitText(state.exit)}`;
  }
};
