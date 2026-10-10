import * as v from "valibot";
import type { Event as DaemonEvent } from "@contracts/Event";
import { INTERNAL_CODE, projectSchema, RpcError } from "../app/seam";
import type { AppSeam, DaemonExit } from "../app/seam";

const outcomeSchema = v.union([
  v.object({ result: v.unknown() }),
  v.object({ error: v.object({ code: v.number(), message: v.string() }) }),
]);

const openSchema = v.object({ project: projectSchema, proof: v.string() });

const frameSchema = v.union([
  v.object({ ready: v.literal(true) }),
  v.object({ event: v.unknown() }),
  v.object({ exited: v.nullable(v.number()) }),
]);

/**
 * The AppSeam of `harness.html?daemon=1` (U144): `rpc` and `subscribe` go to the dev server's `/__daemon/` routes,
 * which relay them to a running `rupd`; the choosers answer `null` and the Dock badge does nothing.
 */
export const createServedApp = (base: string): AppSeam => {
  const exits = new Set<(exit: DaemonExit) => void>();
  let exited: DaemonExit | null = null;

  const exit = (code: number | null) => {
    if (exited) return;

    exited = { code };
    exits.forEach((listener) => listener({ code }));
  };

  const open = (async () => {
    const response = await fetch(`${base}/__daemon/open`);

    return v.parse(openSchema, await response.json());
  })();

  const reach = async <Result>(work: () => Promise<Result>): Promise<Result> => {
    try {
      return await work();
    } catch (raw) {
      if (raw instanceof RpcError) throw raw;

      throw new RpcError(INTERNAL_CODE, raw instanceof Error ? raw.message : String(raw));
    }
  };

  const rpc: AppSeam["rpc"] = (method, params) =>
    reach(async () => {
      const response = await fetch(`${base}/__daemon/rpc`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ method, params }),
      });

      const outcome = v.parse(outcomeSchema, await response.json());

      if ("error" in outcome) throw new RpcError(outcome.error.code, outcome.error.message);

      // SAFETY: the Daemon's result for `method` is the generated contract type, as the Tauri adapter trusts it.
      return outcome.result as never;
    });

  return {
    project: () => reach(async () => (await open).project),
    openProject: (path) =>
      reach(async () => {
        const { project } = await open;

        if (path !== project.path) throw new RpcError(-32003, "a Project is already open");

        return project;
      }),
    daemonProof: () =>
      reach(async () => {
        if (exited) throw new RpcError(INTERNAL_CODE, "the Daemon exited");

        return (await open).proof;
      }),
    chooseProjectPath: async () => null,
    chooseSavePath: async () => null,
    setDockBadge: async () => {},
    rpc,
    subscribe: (onEvent) =>
      reach(
        () =>
          new Promise<void>((ready, fail) => {
            void (async () => {
              try {
                const response = await fetch(`${base}/__daemon/events`);
                const reader = response.body!.pipeThrough(new TextDecoderStream()).getReader();
                let pending = "";
                let subscribed = false;

                for (;;) {
                  const { value, done } = await reader.read();

                  if (done) break;
                  pending += value;

                  for (let end = pending.indexOf("\n"); end >= 0; end = pending.indexOf("\n")) {
                    const frame = v.parse(frameSchema, JSON.parse(pending.slice(0, end)));

                    pending = pending.slice(end + 1);

                    if ("ready" in frame) {
                      subscribed = true;
                      ready();
                    } else if ("event" in frame) {
                      // SAFETY: the Daemon's pushed Events are the generated contract type, as the Tauri adapter trusts them.
                      onEvent(frame.event as DaemonEvent);
                    } else exit(frame.exited);
                  }
                }

                if (!subscribed) fail(new RpcError(INTERNAL_CODE, "the Daemon closed before the subscription"));
                exit(null);
              } catch (error) {
                fail(error);
                exit(null);
              }
            })();
          }),
      ),
    onFileDrop: async () => () => {},
    onDaemonExited: async (listener) => {
      exits.add(listener);

      if (exited) listener(exited);

      return () => void exits.delete(listener);
    },
  };
};
