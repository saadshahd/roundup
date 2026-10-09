import { cleanup, screen } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { DaemonExit } from "../app/seam";
import { info, terminal } from "../testing/nodes";
import { callsTo, mountPane, output } from "./paneHarness";

const savedKey = "roundup:terminal-font:/p";

const press = (target: Element, key: string, options: KeyboardEventInit = {}) => {
  const event = new KeyboardEvent("keydown", { bubbles: true, cancelable: true, metaKey: true, key, ...options });
  target.dispatchEvent(event);

  return event;
};

const mount = async (exit: () => DaemonExit | null = () => null) => {
  const mounted = await mountPane([terminal("a"), terminal("b")], [info("t-a"), info("t-b")], 0, exit);
  mounted.connected.rail.select("a");
  // Snapshot parsing schedules reflow in screens.ts; finish it before exercising a size chord.
  await Promise.resolve();
  vi.advanceTimersToNextFrame();

  return { ...mounted, emulator: mounted.emulators.get("t-a")!, target: mounted.container.querySelector("[data-terminal]")! };
};

beforeEach(() => {
  localStorage.clear();
  vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] });
});

afterEach(() => { cleanup(); vi.useRealTimers(); vi.restoreAllMocks(); localStorage.clear(); });

it.each([
  ["=", false, 14], ["=", true, 14], ["+", false, 14], ["+", true, 14], ["-", false, 12], ["0", false, 13],
])("u102_command_%s_shift_%s_sets_size_%s", async (key, shiftKey, expected) => {
  const { emulator, target, app } = await mount();
  const typed = vi.fn();
  target.addEventListener("keydown", typed);
  expect(press(target, key, { shiftKey }).defaultPrevented).toBe(true);
  vi.advanceTimersToNextFrame();
  expect(emulator.fontSize).toBe(expected);
  expect(typed).not.toHaveBeenCalled();
  expect(callsTo(app, "terminal.write")).toEqual([]);
});

it.each(["=", "+", "-", "0"])("u102_%s_ignores_other_modifier_sets", async (key) => {
  const { emulator, target } = await mount();
  const modifiers: KeyboardEventInit[] = [{ metaKey: false }, { ctrlKey: true }, { altKey: true }];

  if (key === "-" || key === "0") modifiers.push({ shiftKey: true });

  for (const options of modifiers) expect(press(target, key, options).defaultPrevented).toBe(false);
  vi.advanceTimersToNextFrame();
  expect(emulator.fontSize).toBe(13);
});

it.each([["=", 21], ["-", 9]])("u102_repeated_%s_stops_at_%s", async (key, bound) => {
  const { emulator, target, app } = await mount();

  for (let index = 0; index < 20; index += 1) press(target, key, { repeat: index > 0 });
  vi.advanceTimersToNextFrame();
  const fits = emulator.fits;
  app.calls.length = 0;
  expect(press(target, key).defaultPrevented).toBe(true);
  vi.advanceTimersToNextFrame();
  expect(emulator.fontSize).toBe(bound);
  expect(emulator.fits).toBe(fits);
  expect(callsTo(app, "terminal.resize")).toEqual([]);
});

it("u102_zero_restores_the_default", async () => {
  localStorage.setItem(savedKey, "19");
  const { emulator, target } = await mount();
  press(target, "0");
  vi.advanceTimersToNextFrame();
  expect(emulator.fontSize).toBe(13);
});

it("u102_size_is_shared_with_existing_and_later_terminals", async () => {
  const { emulator, target, connected, emulators } = await mount();
  press(target, "=");
  vi.advanceTimersToNextFrame();
  connected.rail.select("b");
  expect(emulators.get("t-b")!.fontSize).toBe(14);
  press(document.querySelector("[data-terminal='t-b']")!, "=");
  vi.advanceTimersToNextFrame();
  connected.rail.select("a");
  expect(emulator.fontSize).toBe(15);
});

it.each([true, false])("u102_changed_fit_%s_sends_only_the_required_resize", async (changed) => {
  const { emulator, target, app } = await mount();
  app.calls.length = 0;

  if (changed) emulator.size = { cols: 90, rows: 25 };
  press(target, "=");
  press(target, "=", { repeat: true });
  vi.advanceTimersToNextFrame();
  expect(callsTo(app, "terminal.resize")).toEqual(changed ? [{ id: "t-a", cols: 90, rows: 25 }] : []);
});

it("u102_size_and_window_resize_share_one_frame", async () => {
  const { emulator, target, app } = await mount();
  app.calls.length = 0;
  emulator.size = { cols: 90, rows: 25 };
  press(target, "=");
  window.dispatchEvent(new Event("resize"));
  vi.advanceTimersToNextFrame();
  expect(callsTo(app, "terminal.resize")).toEqual([{ id: "t-a", cols: 90, rows: 25 }]);
});

it("u102_size_keeps_output_and_scrollback_in_the_same_emulator", async () => {
  const { emulator, target, app, emulators } = await mount();
  app.emit(output("t-a", "first\r\nsecond\r\n"));
  emulator.scroll(false);
  const written = [...emulator.written];
  press(target, "=");
  vi.advanceTimersToNextFrame();
  expect(emulators.get("t-a")).toBe(emulator);
  expect(emulator.written).toEqual(written);
  expect(emulator.isAtBottom()).toBe(false);
  expect(emulator.scrollsToBottom).toBe(0);
});

it.each(["outside", "input", "textarea", "editable"])("u102_leaves_%s_focus_alone", async (place) => {
  const { container, emulator } = await mount();
  const field = document.createElement(place === "editable" ? "div" : place === "input" ? "input" : "textarea");

  if (place === "editable") field.setAttribute("contenteditable", "true");
  (place === "outside" ? container : container.querySelector(".pane-screen")!).append(field);
  field.focus();
  expect(press(field, "=").defaultPrevented).toBe(false);
  vi.advanceTimersToNextFrame();
  expect(emulator.fontSize).toBe(13);
});

it("u102_daemon_exited_prevents_size_changes", async () => {
  const [exit, setExit] = createSignal<DaemonExit | null>(null);
  const { emulator, target, app } = await mount(exit);
  app.calls.length = 0;
  setExit({ code: 1 });
  expect(press(target, "=").defaultPrevented).toBe(false);
  vi.advanceTimersToNextFrame();
  expect(emulator.fontSize).toBe(13);
  expect(callsTo(app, "terminal.resize")).toEqual([]);
  expect(localStorage.getItem(savedKey)).toBeNull();
});

it.each([["9", 9], ["21", 21], ["17", 17], ["8", 13], ["22", 13], ['"17"', 13], ["null", 13], ["{}", 13], ["broken", 13]])(
  "u102_saved_%s_mounts_at_%s", async (saved, expected) => {
    localStorage.setItem(savedKey, saved);
    expect((await mount()).emulator.fontSize).toBe(expected);
  },
);

it("u102_size_survives_remount", async () => {
  const { target } = await mount();
  press(target, "=");
  vi.advanceTimersToNextFrame();
  cleanup();
  expect((await mount()).emulator.fontSize).toBe(14);
});

it("u102_storage_failure_keeps_size_and_shows_the_error", async () => {
  const { emulator, target } = await mount();
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("storage full"); });
  press(target, "=");
  vi.advanceTimersToNextFrame();
  expect(emulator.fontSize).toBe(14);
  expect(screen.getByText("storage full").closest(".pane-failure")).not.toBeNull();
});
