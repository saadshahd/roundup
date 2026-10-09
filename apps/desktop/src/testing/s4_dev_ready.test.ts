import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, expect, it } from "vitest";
import { createServer } from "vite";
import type { ViteDevServer } from "vite";

let root: string;

let server: ViteDevServer;

let url: string;

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), "roundup-s4-"));
  await mkdir(join(root, "src"));
  await writeFile(join(root, "index.html"), '<script type="module" src="/src/main.tsx"></script>');
  await writeFile(join(root, "src/main.tsx"), 'import "./nested";');
  server = await createServer({
    configFile: join(process.cwd(), "vite.config.ts"),
    root,
    logLevel: "silent",
    server: { host: "127.0.0.1", port: 0 },
    optimizeDeps: { noDiscovery: true, include: [] },
  });
  await server.listen();
  url = server.resolvedUrls!.local[0]!;
});

afterEach(async () => {
  await server?.close();
  await rm(root, { recursive: true, force: true });
});

it("s4_html_success_does_not_hide_a_broken_nested_import", async () => {
  await writeFile(join(root, "src/nested.ts"), 'import "missing-s4-dependency";');
  const html = await fetch(url, { signal: AbortSignal.timeout(5000) });
  expect(html.status).toBe(200);
  const result = await fetch(`${url}__roundup_ready`, { signal: AbortSignal.timeout(5000) });
  expect(result.status).toBe(500);
  expect(await result.text()).toContain("missing-s4-dependency");
});

it("s4_ready_accepts_a_transformed_import_cycle", async () => {
  await writeFile(join(root, "src/nested.ts"), 'import "./main";');
  const result = await fetch(`${url}__roundup_ready`, { signal: AbortSignal.timeout(5000) });
  expect(result.status).toBe(204);
});
