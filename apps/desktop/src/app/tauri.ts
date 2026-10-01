import { Channel, invoke } from "@tauri-apps/api/core";
import type { InvokeArgs } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import * as v from "valibot";
import type { Event as DaemonEvent } from "@contracts/Event";
import { daemonExitSchema, INTERNAL_CODE, projectSchema, RpcError } from "./seam";
import type { AppSeam } from "./seam";

const parseFailure = v.safeParser(v.object({ code: v.number(), message: v.string() }));

/**
 * The type argument is trusted, not checked, so use it only for `rpc` and `subscribe`: their types are
 * generated from the Daemon's own Rust types, so a mismatch is a contract bug, not bad input. Everything else
 * is parsed. A rejection that is not `{code, message}` becomes INTERNAL, so callers never see a bare string.
 */
const command = async <Result>(name: string, args: InvokeArgs): Promise<Result> => {
  try {
    return await invoke<Result>(name, args);
  } catch (raw) {
    const failure = parseFailure(raw);

    throw failure.success
      ? new RpcError(failure.output.code, failure.output.message)
      : new RpcError(INTERNAL_CODE, String(raw));
  }
};

export const createTauriApp = (): AppSeam => ({
  project: async () => v.parse(v.nullable(projectSchema), await command("project", {})),
  openProject: async (path) => v.parse(projectSchema, await command("open_project", { path })),
  chooseProjectPath: () => open({ directory: true, multiple: false }),
  chooseSavePath: (suggestedName) => save({ defaultPath: suggestedName }),
  setDockBadge: (count) => getCurrentWindow().setBadgeCount(count === 0 ? undefined : count),
  rpc: (method, params) => command("rpc", { method, params }),
  subscribe: async (onEvent) => {
    await command("subscribe", { channel: new Channel<DaemonEvent>(onEvent) });
  },
  onDaemonExited: (listener) =>
    listen("daemon-exited", (event) => listener(v.parse(daemonExitSchema, event.payload))),
});
