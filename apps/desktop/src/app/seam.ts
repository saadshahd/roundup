import type { Event as DaemonEvent } from "@contracts/Event";
import type { RpcMethodName, RpcMethods } from "@contracts/methods";

export type Project = { name: string; path: string };

export type DaemonExit = { code: number | null };

export type Unsubscribe = () => void;

/** The Tauri commands of `scenarios/app.md`, plus the macOS chooser; the one thing the webview knows about the App. */
export type AppSeam = {
  project(): Promise<Project | null>;
  openProject(path: string): Promise<Project>;
  /** Resolves `null` when the user cancels the macOS chooser. */
  chooseProjectPath(): Promise<string | null>;
  rpc<M extends RpcMethodName>(
    method: M,
    params: RpcMethods[M]["params"],
  ): Promise<RpcMethods[M]["result"]>;
  /** Every Event the Daemon emits from now on arrives at `onEvent`, in the Daemon's order. */
  subscribe(onEvent: (event: DaemonEvent) => void): Promise<void>;
  onDaemonExited(listener: (exit: DaemonExit) => void): Promise<Unsubscribe>;
};

export const INTERNAL_CODE = -32603;

/** A failed call, carrying the Daemon's own `code` and `message`. */
export class RpcError extends Error {
  readonly code: number;

  constructor(code: number, message: string) {
    super(message);
    this.name = "RpcError";
    this.code = code;
  }
}
