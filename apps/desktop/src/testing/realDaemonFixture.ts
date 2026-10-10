import { spawn, spawnSync } from "node:child_process";
import type { ChildProcess } from "node:child_process";
import { mkdir, mkdtemp, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createServer } from "vite";
import type { ViteDevServer } from "vite";
import { realDaemon } from "./realDaemonPlugin";

const REPO = resolve(process.cwd(), "../..");

export const PROOF = "0".repeat(64);

/** A running `rupd` of a fresh Project and the dev server that serves it to the harness page (U144, U146). */
export type StartedDaemon = { daemon: ChildProcess; dir: string; base: string; restart: () => Promise<void>; stop: () => Promise<void> };

const served = async (base: string) => {
  for (let tries = 0; tries < 300; tries += 1) {
    const response = await fetch(`${base}/__daemon/open`).catch(() => null);

    if (response?.ok) return;
    await new Promise((done) => setTimeout(done, 100));
  }

  throw new Error("the dev server never served the Daemon routes");
};

const LOCK = join(tmpdir(), "roundup-cargo-build.lock");

/** Workers build in parallel and a fresh toolchain installs itself on first use, so one worker at a time builds. */
const buildOnce = async () => {
  for (;;) {
    try {
      await mkdir(LOCK);
      break;
    } catch {
      const age = await stat(LOCK).then((lock) => Date.now() - lock.mtimeMs, () => 0);

      if (age > 600_000) await rm(LOCK, { recursive: true, force: true });
      await new Promise((done) => setTimeout(done, 200));
    }
  }

  try {
    const build = () => spawnSync("cargo", ["build", "-p", "rupd", "-p", "rup"], { cwd: REPO, stdio: "inherit" });

    if (build().status !== 0 && build().status !== 0) throw new Error("cargo build -p rupd -p rup failed");
  } finally {
    await rm(LOCK, { recursive: true, force: true });
  }
};

/** Starts `rupd` on the fake `claude` with `env` added to its environment, as `just harness-real` does. */
export const startRealDaemon = async (env: Record<string, string>): Promise<StartedDaemon> => {
  await buildOnce();

  const dir = await mkdtemp(join(tmpdir(), "roundup-real-"));
  const socket = join(dir, "rupd.sock");

  const start = async () => {
    const child = spawn(join(REPO, "target/debug/rupd"), [dir, "--attached"], {
      env: {
        ...process.env,
        RUPD_SOCKET: socket,
        CLAUDE_CONFIG_DIR: join(dir, "claude-config"),
        ROUNDUP_RUP_BIN: join(REPO, "target/debug/rup"),
        ROUNDUP_CLAUDE_BIN: join(REPO, "crates/rup/tests/e2e/fake_claude.py"),
        ...env,
      },
      stdio: ["pipe", "pipe", "pipe"],
    });

    child.stdin!.write(`roundup-proof 1 ${PROOF}\n`);
    await new Promise<void>((serving, fail) => {
      child.stderr!.on("data", (chunk: Buffer) => chunk.toString().includes("serving") && serving());
      child.on("exit", () => fail(new Error("rupd exited before serving")));
    });

    return child;
  };

  let daemon = await start();

  const server: ViteDevServer = await createServer({
    configFile: join(process.cwd(), "vite.config.ts"),
    root: process.cwd(),
    logLevel: "silent",
    plugins: [realDaemon({ project: dir, socket, proof: PROOF })],
    server: { host: "127.0.0.1", port: 0 },
    optimizeDeps: { noDiscovery: true, include: [] },
  });

  await server.listen();

  const base = server.resolvedUrls!.local[0]!.replace(/\/$/, "");

  await served(base);

  return {
    get daemon() {
      return daemon;
    },
    dir,
    base,
    // U177: the Daemon ends and a new one serves the same Project on the same socket.
    restart: async () => {
      const old = daemon;
      const gone = new Promise((done) => old.once("exit", done));

      old.kill();
      await gone;
      daemon = await start();
    },
    stop: async () => {
      daemon.kill();
      await server.close();
      await rm(dir, { recursive: true, force: true });
    },
  };
};
