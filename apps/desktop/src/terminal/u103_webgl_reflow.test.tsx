import * as xtermModule from "@xterm/xterm";
import { Terminal } from "@xterm/xterm";
import { expect, it, vi } from "vitest";
import { createXtermEmulators } from "./emulator";

const webgl = vi.hoisted(() => ({ disposed: 0 }));

vi.mock("@xterm/addon-webgl", () => ({
  WebglAddon: class {
    activate() {}
    dispose() { webgl.disposed += 1; }
    onContextLoss() { return { dispose() {} }; }
  },
}));

it("u103_a_snapshot_resize_of_an_open_webgl_emulator_falls_back_to_the_dom_renderer", () => {
  webgl.disposed = 0;
  const spy = vi.spyOn(xtermModule, "Terminal");
  const emulator = createXtermEmulators()("t-a");
  // SAFETY: the factory created exactly one Terminal while the spy was attached.
  const terminal = spy.mock.instances[0] as Terminal;

  spy.mockRestore();
  vi.stubGlobal("matchMedia", () => ({ matches: false, addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {} }));
  emulator.show(document.createElement("div"));
  emulator.setSize({ cols: terminal.cols, rows: terminal.rows });
  expect(webgl.disposed).toBe(0);

  emulator.setSize({ cols: terminal.cols + 1, rows: terminal.rows });
  expect(webgl.disposed).toBe(1);
  emulator.dispose();
  vi.unstubAllGlobals();
});
