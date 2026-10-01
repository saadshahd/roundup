import solid from "vite-plugin-solid";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [solid()],
  // tauri.conf.json's devUrl names this port; moving off it must fail, not open a blank window.
  server: { port: 5173, strictPort: true },
  test: {
    environment: "jsdom",
    // Vitest returns empty CSS by default; the Drawer test needs the real rules to read computed positions.
    css: { include: [/styles\.css/] },
    include: ["src/**/*.test.{ts,tsx}"],
    // jsdom resolves the server build of solid-js unless told to use the browser one.
    server: { deps: { inline: [/solid-js/] } },
  },
  resolve: { conditions: ["browser"] },
});
