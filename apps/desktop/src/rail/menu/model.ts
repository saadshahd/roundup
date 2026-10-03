import { createSignal } from "solid-js";
import type { RailNode } from "@contracts/agent/RailNode";
import { RpcError } from "../../app/seam";
import type { AppSeam } from "../../app/seam";
import type { ExitState, RailState } from "../../state/rail";

type MenuState = { node: RailNode; exit: ExitState | null; x: number; y: number; confirming: boolean };

const isAgent = (node: RailNode) => node.kind === "agent" || node.meta;

const isRunning = (node: RailNode, exit: ExitState | null) =>
  exit === null && (node.kind === "terminal" || (isAgent(node) && node.status?.kind !== "done"));

const needsConfirmation = ({ node, exit }: MenuState) =>
  isRunning(node, exit) &&
  (node.kind === "terminal" ||
    node.status?.kind === "working" ||
    node.status?.kind === "needs-you" ||
    node.status?.kind === "blocked");

export const createRailMenu = (app: AppSeam, rail: RailState, onFailure: (message: string) => void) => {
  const [state, setState] = createSignal<MenuState | null>(null);

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

  const chooseRemove = () => {
    const current = state();

    if (!current) return;

    if (!current.confirming && needsConfirmation(current)) {
      setState({ ...current, confirming: true });

      return;
    }

    void attempt(async () => {
      if (isRunning(current.node, current.exit)) await stop(current.node);

      await app.rpc("rail.remove", { id: current.node.id });
    });
  };

  return { state, open, close, chooseStop, chooseRemove, canStop: (current: MenuState) => isRunning(current.node, current.exit) };
};

export type RailMenuModel = ReturnType<typeof createRailMenu>;
