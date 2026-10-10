import * as v from "valibot";
import type { Event as DaemonEvent } from "@contracts/Event";
import type { RpcMethodName, RpcMethods } from "@contracts/methods";

/** Hand-written App seam types (`scenarios/app.md`), not generated from Rust, so the adapter parses them. */
export const projectSchema = v.object({ name: v.string(), path: v.string() });

export const daemonExitSchema = v.object({ code: v.nullable(v.number()) });

export type Project = v.InferOutput<typeof projectSchema>;

export type DaemonExit = v.InferOutput<typeof daemonExitSchema>;

export type Unsubscribe = () => void;

/** One step of a File drop: `x` and `y` in CSS pixels of the webview; `paths` absolute, empty unless `phase` is `drop`. */
export type FileDrop = { phase: "over" | "drop" | "leave"; paths: string[]; x: number; y: number };

/** The Tauri commands of `scenarios/app.md`, plus the macOS choosers and the Dock badge; the only door from the webview to Tauri. */
export type AppSeam = {
  project(): Promise<Project | null>;
  openProject(path: string): Promise<Project>;
  /** The proof the App gave the open Project's Daemon, which `decision.answer` needs (H4); keep it in memory only. Rejects before a Project is open and after its Daemon exits. */
  daemonProof(): Promise<string>;
  /** Resolves `null` when the user cancels the macOS chooser. */
  chooseProjectPath(): Promise<string | null>;
  /** The macOS save chooser, with `suggestedName` filled in; resolves `null` on cancel. */
  chooseSavePath(suggestedName: string): Promise<string | null>;
  /** Shows `count` on the Dock icon; zero clears the badge. */
  setDockBadge(count: number): Promise<void>;
  rpc<M extends RpcMethodName>(
    method: M,
    params: RpcMethods[M]["params"],
  ): Promise<RpcMethods[M]["result"]>;
  /** Every Event the Daemon emits from now on arrives at `onEvent`, in the Daemon's order. */
  subscribe(onEvent: (event: DaemonEvent) => void): Promise<void>;
  /** Reports the window's native File drop, which Tauri takes before the webview sees a DOM `drop`. */
  onFileDrop(listener: (drop: FileDrop) => void): Promise<Unsubscribe>;
  onDaemonExited(listener: (exit: DaemonExit) => void): Promise<Unsubscribe>;
};

export const INTERNAL_CODE = -32603;

/** A failed call: the Daemon's own `code` and `message`, or INTERNAL when Tauri, a plugin or a malformed answer failed. */
export class RpcError extends Error {
  readonly code: number;

  constructor(code: number, message: string) {
    super(message);
    this.name = "RpcError";
    this.code = code;
  }
}
