import { createSignal } from "solid-js";
import type { RailNode } from "@contracts/agent/RailNode";
import { RpcError } from "../../app/seam";
import type { AppSeam } from "../../app/seam";
import type { ExitState, RailState } from "../../state/rail";

type MenuState = { node: RailNode; exit: ExitState | null; x: number; y: number; confirming: boolean };

const isAgent = (node: RailNode) => node.kind !== "terminal";

const NOT_FOUND = -32001;

const UNKNOWN_OUTCOME = -32004;

/** U68: an Agent runs while its Terminal does, whatever its Kind; a Room keeps U56's Kind rule. */
const isRunning = (node: RailNode, exit: ExitState | null) =>
  exit === null && (node.kind === "terminal" || (node.kind === "agent" ? true : node.status?.kind !== "done"));

const needsConfirmation = ({ node, exit }: MenuState) =>
  isRunning(node, exit) &&
  (node.kind === "terminal" ||
    node.status?.kind === "working" ||
    node.status?.kind === "needs-you" ||
    node.status?.kind === "blocked");

export const createRailMenu = (app: AppSeam, rail: RailState, onFailure: (message: string) => void) => {
  const [state, setState] = createSignal<MenuState | null>(null);
  const resuming = new Set<string>();

  const close = () => setState(null);

  const open = (node: RailNode, exit: ExitState | null, x: number, y: number) => {
    rail.select(node.id);
    setState({ node, exit, x, y, confirming: false });
  };

  const attempt = async (call: () => Promise<void>) => {
    close();

    try {
      await call();
    } catch (error) {
      if (error instanceof RpcError && error.code === -32001) {
        await rail.refresh();
      } else if (error instanceof Error) {
        onFailure(error.message);
      } else {
        throw error;
      }
    }
  };

  const stop = (node: RailNode) =>
    isAgent(node)
      ? app.rpc("agent.stop", { id: node.id })
      : node.terminal_id !== null
        ? app.rpc("terminal.kill", { id: node.terminal_id })
        : Promise.resolve(null);

  const chooseStop = () => {
    const current = state();

    if (current) void attempt(async () => { await stop(current.node); });
  };

  const resume = async (id: string) => {
    if (resuming.has(id)) return;

    resuming.add(id);

    try {
      await app.rpc("agent.resume", { id });
      await rail.refresh();
    } catch (error) {
      if (!(error instanceof RpcError)) {
        if (error instanceof Error) onFailure(error.message);
        else throw error;
      } else if (error.code === UNKNOWN_OUTCOME) {
        await rail.refresh({ terminals: true });

        if (rail.failure() !== null) onFailure("resume outcome unknown: the Daemon may have resumed it");
      } else {
        onFailure(error.message);

        if (error.code === NOT_FOUND) await rail.refresh();
      }
    } finally {
      resuming.delete(id);
    }
  };

  const chooseResume = () => {
    const current = state();

    close();

    if (current) void resume(current.node.id);
  };

  const chooseRemove = () => {
    const current = state();

    if (!current) return;

    if (!current.confirming && needsConfirmation(current)) {
      setState({ ...current, confirming: true });

      return;
    }

    void attempt(async () => {
      await app.rpc("rail.remove", { id: current.node.id });
    });
  };

  return {
    state,
    open,
    close,
    chooseStop,
    chooseRemove,
    chooseResume,
    canStop: (current: MenuState) => isRunning(current.node, current.exit),
    canResume: (current: MenuState) => current.node.can_resume && !isRunning(current.node, current.exit),
  };
};

export type RailMenuModel = ReturnType<typeof createRailMenu>;
