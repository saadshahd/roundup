import { Channel, invoke } from "@tauri-apps/api/core";
import type { InvokeArgs } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import * as v from "valibot";
import type { Event as DaemonEvent } from "@contracts/Event";
import { daemonExitSchema, INTERNAL_CODE, projectSchema, RpcError } from "./seam";
import type { AppSeam } from "./seam";

const parseFailure = v.safeParser(v.object({ code: v.number(), message: v.string() }));

/**
 * Every request the adapter makes into Tauri goes through here, so a failure reaches the webview as an RpcError:
 * a command rejects with `{code, message}`; the dialog, window and event plugins reject with a plain string,
 * which becomes INTERNAL. Anything else thrown inside `work` (a malformed answer) is INTERNAL with its message.
 */
const guarded = async <Result>(work: () => Promise<Result>): Promise<Result> => {
  try {
    return await work();
  } catch (raw) {
    const failure = parseFailure(raw);

    throw failure.success
      ? new RpcError(failure.output.code, failure.output.message)
      : new RpcError(INTERNAL_CODE, raw instanceof Error ? raw.message : String(raw));
  }
};

/**
 * The type argument is trusted, not checked, so use it only for `rpc` and `subscribe`: their types are
 * generated from the Daemon's own Rust types, so a mismatch is a contract bug, not bad input.
 */
const command = <Result>(name: string, args: InvokeArgs): Promise<Result> =>
  guarded(() => invoke<Result>(name, args));

/** For the hand-written App seam types: the answer is parsed, and a malformed one is an INTERNAL RpcError. */
const parsedCommand = <Schema extends v.GenericSchema>(
  schema: Schema,
  name: string,
  args: InvokeArgs,
): Promise<v.InferOutput<Schema>> => guarded(async () => v.parse(schema, await invoke(name, args)));

/** Tauri reports the drop at physical pixels; the webview lays out in CSS pixels. */
export const tauriFileDrop = (
  webview: () => Pick<ReturnType<typeof getCurrentWebview>, "onDragDropEvent">,
  ratio: () => number,
): AppSeam["onFileDrop"] => (listener) =>
  guarded(() =>
    webview().onDragDropEvent(({ payload }) => {
      if (payload.type === "leave") {
        listener({ phase: "leave", paths: [], x: 0, y: 0 });

        return;
      }

      const scale = ratio() || 1;

      listener({
        phase: payload.type === "drop" ? "drop" : "over",
        paths: payload.type === "drop" ? payload.paths : [],
        x: payload.position.x / scale,
        y: payload.position.y / scale,
      });
    }),
  );

export const createTauriApp = (): AppSeam => ({
  project: () => parsedCommand(v.nullable(projectSchema), "project", {}),
  openProject: (path) => parsedCommand(projectSchema, "open_project", { path }),
  daemonProof: () => parsedCommand(v.string(), "daemon_proof", {}),
  chooseProjectPath: () => guarded(() => open({ directory: true, multiple: false })),
  chooseSavePath: (suggestedName) => guarded(() => save({ defaultPath: suggestedName })),
  setDockBadge: (count) => guarded(() => getCurrentWindow().setBadgeCount(count === 0 ? undefined : count)),
  rpc: (method, params) => command("rpc", { method, params }),
  subscribe: async (onEvent) => {
    await command("subscribe", { channel: new Channel<DaemonEvent>(onEvent) });
  },
  onFileDrop: tauriFileDrop(getCurrentWebview, () => window.devicePixelRatio),
  onDaemonExited: (listener) =>
    guarded(() => listen("daemon-exited", (event) => listener(v.parse(daemonExitSchema, event.payload)))),
});
