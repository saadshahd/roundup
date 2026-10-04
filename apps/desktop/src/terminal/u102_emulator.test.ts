import * as xtermModule from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { afterEach, expect, it, vi } from "vitest";
import { createXtermEmulators } from "./emulator";
import { fontSizeStorage } from "./fontSize";

afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); localStorage.clear(); });

it.each([false, true])("u102_xterm_keeps_scrollback_and_viewport_at_bottom_%s", async (bottom) => {
  const spy = vi.spyOn(xtermModule, "Terminal");

  const emulator = createXtermEmulators(undefined, () => ({
    activate() {}, dispose() {}, onContextLoss() { return { dispose() {} }; },
  }))("t-a");

  const terminal = spy.mock.instances[0];

  if (!terminal) throw new Error("xterm was not created");
  spy.mockRestore();
  const lines = Array.from({ length: 80 }, (_, index) => `line ${index}`).join("\r\n");
  await new Promise<void>((resolve) => emulator.write(new TextEncoder().encode(lines), resolve));

  if (!bottom) terminal.scrollToLine(8);
  const viewport = terminal.buffer.active.viewportY;
  const content = () => Array.from({ length: terminal.buffer.active.length }, (_, index) => terminal.buffer.active.getLine(index)?.translateToString(true));
  const before = content();
  vi.spyOn(FitAddon.prototype, "fit").mockImplementation(() => terminal.resize(60, 20));
  emulator.setFontSize(15);
  emulator.fit();
  expect(terminal.options.fontSize).toBe(15);
  expect(content()).toEqual(before);
  expect(terminal.buffer.active.viewportY).toBe(bottom ? terminal.buffer.active.baseY : viewport);
  emulator.dispose();
});

it("u102_another_project_does_not_read_the_saved_size", () => {
  fontSizeStorage("/one").write(18);
  expect(fontSizeStorage("/two").read()).toBeNull();
  expect(fontSizeStorage("/one").read()).toBe(18);
});

it("u102_unreadable_storage_is_absent", () => {
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw new Error("denied"); });
  expect(fontSizeStorage("/p").read()).toBeNull();
});

it.each([[100, 93, 60], [93, 100, 90], [93, 100, 91]])("u102_rewrap_%s_to_%s_keeps_logical_line_at_row_%s", async (beforeCols, afterCols, row) => {
  const spy = vi.spyOn(xtermModule, "Terminal");

  const emulator = createXtermEmulators(undefined, () => ({
    activate() {}, dispose() {}, onContextLoss() { return { dispose() {} }; },
  }))("t-a");

  const terminal = spy.mock.instances[0];

  if (!terminal) throw new Error("xterm was not created");
  spy.mockRestore();
  terminal.resize(beforeCols, 24);
  const lines = Array.from({ length: 100 }, (_, index) => `${String(index).padStart(3, "0")} ${"x".repeat(190)}`).join("\r\n");
  await new Promise<void>((resolve) => emulator.write(new TextEncoder().encode(lines), resolve));
  terminal.scrollToLine(row);

  const top = () => {
    const buffer = terminal.buffer.active;
    let line = buffer.viewportY;

    while (line > 0 && buffer.getLine(line)?.isWrapped) line -= 1;

    return buffer.getLine(line)?.translateToString(true);
  };

  expect(top()).toMatch(/^030 /);
  vi.spyOn(FitAddon.prototype, "fit").mockImplementation(() => terminal.resize(afterCols, 24));
  emulator.setFontSize(14);
  emulator.fit();
  expect(top()).toMatch(/^030 /);
  emulator.dispose();
});
