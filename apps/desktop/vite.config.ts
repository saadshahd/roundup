import solid from "vite-plugin-solid";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [solid()],
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    // jsdom resolves the server build of solid-js unless told to use the browser one.
    server: { deps: { inline: [/solid-js/] } },
  },
  resolve: { conditions: ["browser"] },
});
