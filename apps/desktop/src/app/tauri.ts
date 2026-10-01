import { Channel, invoke } from "@tauri-apps/api/core";
import type { InvokeArgs } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import * as v from "valibot";
import type { Event as DaemonEvent } from "@contracts/Event";
import { INTERNAL_CODE, RpcError } from "./seam";
import type { AppSeam, DaemonExit, Project } from "./seam";

const parseFailure = v.safeParser(v.object({ code: v.number(), message: v.string() }));

/**
 * The type argument is trusted, not checked: the App returns the JSON of the Rust types that
 * `contracts/generated` is generated from, so a mismatch is a contract bug, not bad input.
 * A rejection that is not `{code, message}` becomes INTERNAL, so callers never see a bare string.
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
  project: () => command<Project | null>("project", {}),
  openProject: (path) => command<Project>("open_project", { path }),
  chooseProjectPath: () => open({ directory: true, multiple: false }),
  rpc: (method, params) => command("rpc", { method, params }),
  subscribe: async (onEvent) => {
    await command("subscribe", { channel: new Channel<DaemonEvent>(onEvent) });
  },
  onDaemonExited: async (listener) =>
    listen<DaemonExit>("daemon-exited", (event) => listener(event.payload)),
});
