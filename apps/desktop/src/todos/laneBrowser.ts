import { execFile } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";
import { createServer } from "vite";
import type { ViteDevServer } from "vite";
import { captureDir } from "../testing/captures";
import type { Checks } from "../testing/checks";

const exec = promisify(execFile);

/** One headless Chromium on a fresh profile and its own Vite server, driven by `agent-browser` on the U26 harness; never the user's browser. */
export type LaneBrowser = {
  browser: (...args: string[]) => Promise<string>;
  /** Evaluates a constant expression in the page and returns its JSON value. */
  evaluate: <T>(script: string) => Promise<T>;
  /** The focused control's name (its label, else its text) and, after `@`, the Todo whose row holds it. */
  focused: () => Promise<string>;
  /** Tabs from the focused control until `name` holds focus, with real key presses only. */
  tabTo: (name: string) => Promise<void>;
  /** Waits until the page function returns truthy. */
  until: (predicate: string) => Promise<void>;
  /** Writes the Checks and a screenshot through the shared capture directory when enabled. */
  capture: (id: string, step: string) => Promise<void>;
  /** Reloads the harness, makes one Todo per title, the third blocked by the first, and shows the Todos tab. */
  seedTodos: (titles: readonly string[]) => Promise<void>;
  /** The Todo ids in the order the Shelf shows them. */
  order: () => Promise<number[]>;
  /** The `method` of every call the fake Daemon took since the last reset, with its params. */
  calls: (method?: string) => Promise<{ method: string; params: { id: number } }[]>;
  resetCalls: () => Promise<void>;
  close: () => Promise<void>;
};

export const openLane = async (width: number, scheme: "light" | "dark", name: string, reduced = false): Promise<LaneBrowser> => {
  const profile = mkdtempSync(join(tmpdir(), `${name}-browser-`));
  const cacheDir = mkdtempSync(join(tmpdir(), `${name}-vite-`));
  const server: ViteDevServer = await createServer({ root: process.cwd(), cacheDir, server: { host: "127.0.0.1", port: 0, hmr: false }, logLevel: "silent" });

  await server.listen();

  const session = `${name}-${process.pid}-${width}-${scheme}`;
  const browser = async (...args: string[]) => (await exec("agent-browser", ["--session", session, "--profile", profile, ...args], { timeout: 60_000 })).stdout;
  // SAFETY: agent-browser prints the JSON value of the evaluated expression.
  const evaluate = async <T,>(script: string): Promise<T> => JSON.parse(await browser("eval", script)) as T;
  const until = async (predicate: string) => { await browser("wait", "--fn", predicate); };

  await browser("set", "viewport", String(width), width === 1280 ? "800" : "950");
  await browser("set", "media", scheme, ...(reduced ? ["reduced-motion"] : []));

  const reload = async () => {
    // A selected Workstream or scope outlives a reload; each test starts from tree-40 as the harness opens it.
    await browser("eval", "localStorage.clear()").catch(() => undefined);
    await browser("open", `${server.resolvedUrls!.local[0]}harness.html?seed=tree-40`);
    await browser("wait", ".shelf-tabs");
  };

  await reload();

  const focused = () => evaluate<string>("(() => { const el = document.activeElement; const row = el.closest('.todo-row'); return `${el.getAttribute('aria-label') ?? el.textContent.trim()}${row?.dataset.id ? '@' + row.dataset.id : ''}`; })()");

  const tabTo = async (name: string) => {
    for (let step = 0; step < 24 && (await focused()) !== name; step++) await browser("press", "Tab");

    if ((await focused()) !== name) throw new Error(`Tab never reached ${name}; focus is on ${await focused()}`);
  };

  const calls = (method?: string) => evaluate<{ method: string; params: { id: number } }[]>(`window.__fake.app.calls.filter(call => ${method === undefined ? "true" : `call.method === ${JSON.stringify(method)}`}).map(call => ({ method: call.method, params: call.params }))`);

  const resetCalls = async () => { await evaluate("(() => { window.__fake.app.calls.length = 0; return true; })()"); };

  return {
    browser, evaluate, until, calls, resetCalls, focused, tabTo,
    capture: async (id, step) => {
      const dir = captureDir(id);

      if (!dir) return;
      mkdirSync(dir, { recursive: true });
      writeFileSync(join(dir, `${width}x${width === 1280 ? 800 : 950}-${scheme}-${step}.checks.json`), JSON.stringify(await evaluate<Checks>("window.__checks()")) + "\n");
      await browser("screenshot", join(dir, `${width}x${width === 1280 ? 800 : 950}-${scheme}-${step}.png`));
    },
    seedTodos: async (titles) => {
      await reload();
      await evaluate(`(async () => {
        const rpc = window.__fake.app.rpc;
        for (const todo of await rpc('todo.list', null)) await rpc('todo.delete', { id: todo.id });
        window.__fake.actAs({ kind: 'agent', id: 'tokens', parent: null });
        const made = [];
        for (const title of ${JSON.stringify(titles)}) made.push(await rpc('todo.create', { title, body: null, blockers: null }));
        await rpc('todo.setBlockers', { id: made[2].id, blockers: [made[0].id] });
        window.__fake.actAs({ kind: 'user', id: 'you', parent: null });
        [...document.querySelectorAll('[role="tab"]')].find(tab => tab.textContent.startsWith('Todos')).click();
        return true;
      })()`);
      await until("document.querySelectorAll('.shelf .todo-row').length >= " + titles.length);
      await resetCalls();
    },
    order: () => evaluate<number[]>("[...document.querySelectorAll('.shelf .todo-row[data-id]')].map(row => Number(row.dataset.id))"),
    close: async () => {
      try { await browser("close"); } finally { await server.close(); rmSync(profile, { recursive: true, force: true }); rmSync(cacheDir, { recursive: true, force: true }); }
    },
  };
};
