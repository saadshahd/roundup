import solid from "vite-plugin-solid";
import { defineConfig } from "vitest/config";
import { realDaemon, realDaemonFromEnv } from "./src/testing/realDaemonPlugin";

const served = realDaemonFromEnv(process.env);

export default defineConfig({
  plugins: [solid(), ...(served ? [realDaemon(served)] : []), {
    name: "roundup-dev-ready",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/__roundup_ready", async (_request, response) => {
        try {
          const client = server.environments.client;

          if (!client) throw new Error("Vite has no client environment");
          const urls = new Set(["/src/main.tsx"]);

          for (const url of urls) {
            if (response.destroyed) return;

            const transformed = await client.transformRequest(url);
            const module = await client.moduleGraph.getModuleByUrl(url);

            if (!transformed || !module) throw new Error(`Vite could not transform ${url}`);

            for (const imported of module.importedModules) urls.add(imported.url);
          }

          response.writeHead(204).end();
        } catch (error) {
          response.writeHead(500, { "Content-Type": "text/plain" });
          response.end(String(error));
        }
      });
    },
  }],
  // tauri.conf.json's devUrl names this port; moving off it must fail, not open a blank window.
  server: { port: 5173, strictPort: true },
  test: {
    environment: "jsdom",
    // Vitest returns empty CSS by default; the Drawer and tokens tests need the real rules.
    css: { include: [/styles\.css/, /tokens\.css/] },
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["src/testing/domStubs.ts"],
    // jsdom resolves the server build of solid-js unless told to use the browser one.
    server: { deps: { inline: [/solid-js/] } },
  },
  resolve: { conditions: ["browser", "development|production"] },
});
