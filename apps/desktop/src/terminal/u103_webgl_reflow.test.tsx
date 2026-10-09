import * as xtermModule from "@xterm/xterm";
import { Terminal } from "@xterm/xterm";
import { expect, it, vi } from "vitest";
import { createXtermEmulators } from "./emulator";
import type { RendererAddon } from "./emulator";

it("u103_a_snapshot_resize_of_an_open_webgl_emulator_falls_back_to_the_dom_renderer", () => {
  let disposed = 0;

  const createWebgl = (): RendererAddon => ({
    activate() {},
    dispose() { disposed += 1; },
    onContextLoss() { return { dispose() {} }; },
  });

  const spy = vi.spyOn(xtermModule, "Terminal");
  const emulator = createXtermEmulators(undefined, createWebgl)("t-a");
  // SAFETY: the factory created exactly one Terminal while the spy was attached.
  const terminal = spy.mock.instances[0] as Terminal;

  spy.mockRestore();
  vi.stubGlobal("matchMedia", () => ({ matches: false, addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {} }));
  emulator.show(document.createElement("div"));
  emulator.setSize({ cols: terminal.cols, rows: terminal.rows });
  expect(disposed).toBe(0);

  emulator.setSize({ cols: terminal.cols + 1, rows: terminal.rows });
  expect(disposed).toBe(1);
  emulator.dispose();
  vi.unstubAllGlobals();
});
