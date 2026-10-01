import type { ExitState } from "../state/rail";

/** How an exited node reads on screen: its code, a signal, or nothing known because the node has no Terminal. */
export const exitText = (exit: ExitState): string => {
  switch (exit.kind) {
    case "code":
      return `exited ${exit.code}`;
    case "signal":
      return "exited by signal";
    case "unknown":
      return "exited";
  }
};
