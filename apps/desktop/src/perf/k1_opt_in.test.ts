import solid from "vite-plugin-solid";
import { build } from "vite";
import { afterEach, describe, expect, it, vi } from "vitest";

const MARKER = "roundup-keystroke-probe";

const bundled = async (): Promise<string> => {
  const built = await build({
    root: ".",
    configFile: false,
    plugins: [solid()],
    logLevel: "silent",
    build: { write: false },
  });

  const outputs = [built].flat();

  return outputs
    .flatMap((output) => ("output" in output ? output.output : []))
    .map((file) => (file.type === "chunk" ? file.code : ""))
    .join("\n");
};

afterEach(() => vi.unstubAllEnvs());

describe("k1 the keystroke run is opt-in at build time", () => {
  it("k1_a_normal_build_carries_no_trace_of_the_probe", async () => {
    expect(await bundled()).not.toContain(MARKER);
  }, 60_000);

  it("k1_a_build_made_with_VITE_ROUNDUP_PERF_carries_it", async () => {
    vi.stubEnv("VITE_ROUNDUP_PERF", "1");

    expect(await bundled()).toContain(MARKER);
  }, 60_000);
});
