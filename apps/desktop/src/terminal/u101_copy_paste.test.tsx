import { cleanup, screen } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DaemonExit } from "../app/seam";
import { info, terminal } from "../testing/nodes";
import { callsTo, mountPane } from "./paneHarness";

const clipboard = (text = "") => {
  const readText = vi.fn(async () => text);
  const writeText = vi.fn(async (_value: string) => {});

  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { readText, writeText } });

  return { readText, writeText };
};

const key = (target: Element, key: string, options: Record<string, boolean> = {}) => {
  const press = new KeyboardEvent("keydown", { bubbles: true, cancelable: true, key, ...options });

  target.dispatchEvent(press);

  return press;
};

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("u101 Terminal copy and paste", () => {
  it("u101_command_c_copies_only_selected_text_and_control_c_still_types", async () => {
    const clip = clipboard();
    const { app, connected, emulators, container } = await mountPane([terminal("a")], [info("t-a")]);

    connected.rail.select("a");
    const emulator = emulators.get("t-a")!;
    const target = container.querySelector("[data-terminal]")!;
    const emulatedKey = vi.fn();

    target.addEventListener("keydown", emulatedKey);

    emulator.selectedText = "selected text";
    expect(key(target, "c", { metaKey: true }).defaultPrevented).toBe(true);
    await vi.waitFor(() => expect(clip.writeText).toHaveBeenCalledWith("selected text"));
    expect(callsTo(app, "terminal.write")).toEqual([]);
    expect(emulatedKey).not.toHaveBeenCalled();

    emulator.selectedText = "";
    expect(key(target, "c", { metaKey: true }).defaultPrevented).toBe(true);
    expect(clip.writeText).toHaveBeenCalledTimes(1);

    expect(key(target, "c", { ctrlKey: true }).defaultPrevented).toBe(false);
    emulator.type(Uint8Array.of(3));
    await vi.waitFor(() => expect(callsTo(app, "terminal.write")).toEqual([{ id: "t-a", data: "Aw==" }]));
  });

  it("u101_command_v_hands_multiline_text_to_emulator_once", async () => {
    const clip = clipboard("one\ntwo\n");
    const { app, connected, emulators, container } = await mountPane([terminal("a")], [info("t-a")]);

    connected.rail.select("a");
    const emulator = emulators.get("t-a")!;
    const press = key(container.querySelector("[data-terminal]")!, "v", { metaKey: true });

    expect(press.defaultPrevented).toBe(true);
    await vi.waitFor(() => expect(emulator.pasted).toEqual(["one\ntwo\n"]));
    expect(clip.readText).toHaveBeenCalledTimes(1);
    await vi.waitFor(() => expect(callsTo(app, "terminal.write")).toEqual([
      { id: "t-a", data: "b25lCnR3bwo=" },
    ]));
  });

  it("u101_empty_clipboard_and_modified_chords_do_nothing", async () => {
    const clip = clipboard("");
    const { app, connected, emulators, container } = await mountPane([terminal("a")], [info("t-a")]);

    connected.rail.select("a");
    const emulator = emulators.get("t-a")!;
    const target = container.querySelector("[data-terminal]")!;

    emulator.selectedText = "text";
    key(target, "v", { metaKey: true });
    await vi.waitFor(() => expect(clip.readText).toHaveBeenCalledTimes(1));

    for (const chord of [
      { key: "c", shiftKey: true }, { key: "c", altKey: true },
      { key: "v", shiftKey: true }, { key: "v", altKey: true },
    ]) expect(key(target, chord.key, { metaKey: true, shiftKey: chord.shiftKey ?? false, altKey: chord.altKey ?? false }).defaultPrevented).toBe(false);

    expect(emulator.pasted).toEqual([]);
    expect(clip.writeText).not.toHaveBeenCalled();
    expect(callsTo(app, "terminal.write")).toEqual([]);
  });

  it("u101_chords_leave_other_fields_to_the_browser", async () => {
    const clip = clipboard("paste");
    const { connected, emulators, container } = await mountPane([terminal("a")], [info("t-a")]);

    connected.rail.select("a");
    emulators.get("t-a")!.selectedText = "copy";
    const field = document.createElement("textarea");

    container.append(field);
    field.focus();
    expect(key(field, "c", { metaKey: true }).defaultPrevented).toBe(false);
    expect(key(field, "v", { metaKey: true }).defaultPrevented).toBe(false);
    expect(clip.writeText).not.toHaveBeenCalled();
    expect(clip.readText).not.toHaveBeenCalled();

    container.querySelector(".pane-screen")!.append(field);
    field.focus();
    expect(key(field, "c", { metaKey: true }).defaultPrevented).toBe(false);
    expect(key(field, "v", { metaKey: true }).defaultPrevented).toBe(false);
    expect(clip.writeText).not.toHaveBeenCalled();
    expect(clip.readText).not.toHaveBeenCalled();
  });

  it("u101_exited_terminal_and_daemon_exited_copy_but_never_paste", async () => {
    const clip = clipboard("paste");
    const [exit, setExit] = createSignal<DaemonExit | null>(null);

    const { app, connected, emulators, container } = await mountPane(
      [terminal("a"), terminal("b")],
      [info("t-a", { running: false, exit_code: 0 }), info("t-b")],
      0,
      exit,
    );

    connected.rail.select("a");
    emulators.get("t-a")!.selectedText = "exited copy";
    key(container.querySelector("[data-terminal='t-a']")!, "c", { metaKey: true });
    key(container.querySelector("[data-terminal='t-a']")!, "v", { metaKey: true });
    await vi.waitFor(() => expect(clip.writeText).toHaveBeenCalledWith("exited copy"));
    expect(emulators.get("t-a")!.pasted).toEqual([]);

    connected.rail.select("b");
    setExit({ code: 1 });
    emulators.get("t-b")!.selectedText = "daemon copy";
    key(container.querySelector("[data-terminal='t-b']")!, "c", { metaKey: true });
    key(container.querySelector("[data-terminal='t-b']")!, "v", { metaKey: true });
    await vi.waitFor(() => expect(clip.writeText).toHaveBeenCalledWith("daemon copy"));
    expect(emulators.get("t-b")!.pasted).toEqual([]);
    expect(clip.readText).not.toHaveBeenCalled();
    expect(callsTo(app, "terminal.write")).toEqual([]);
  });

  it("u101_paste_waiting_on_clipboard_does_not_write_after_daemon_exits", async () => {
    const clip = clipboard();
    const [exit, setExit] = createSignal<DaemonExit | null>(null);
    const { app, connected, emulators, container } = await mountPane([terminal("a")], [info("t-a")], 0, exit);
    let release: ((text: string) => void) | undefined;

    clip.readText.mockImplementationOnce(() => new Promise((resolve) => { release = resolve; }));
    connected.rail.select("a");
    key(container.querySelector("[data-terminal]")!, "v", { metaKey: true });
    setExit({ code: 1 });
    release?.("late paste");
    await Promise.resolve();

    expect(clip.readText).toHaveBeenCalledTimes(1);
    expect(emulators.get("t-a")!.pasted).toEqual([]);
    expect(callsTo(app, "terminal.write")).toEqual([]);
  });

  it("u101_clipboard_failures_show_in_the_pane_error_slot", async () => {
    const clip = clipboard("paste");
    const { connected, emulators, container } = await mountPane([terminal("a")], [info("t-a")]);

    connected.rail.select("a");
    const target = container.querySelector("[data-terminal]")!;

    emulators.get("t-a")!.selectedText = "copy";
    clip.writeText.mockRejectedValueOnce(new Error("copy denied"));
    key(target, "c", { metaKey: true });
    expect((await screen.findByRole("alert", { name: "copy denied" })).closest(".pane-failure")).not.toBeNull();

    clip.readText.mockRejectedValueOnce(new Error("paste denied"));
    key(target, "v", { metaKey: true });
    expect((await screen.findByRole("alert", { name: "paste denied" })).closest(".pane-failure")).not.toBeNull();
    expect(emulators.get("t-a")!.pasted).toEqual([]);
    expect(screen.getByRole("alert").querySelector('svg.lucide-x[aria-hidden="true"]')).not.toBeNull();
  });

  it("u101_clipboard_failure_belongs_to_its_terminal", async () => {
    const clip = clipboard();

    const { connected, emulators, container } = await mountPane(
      [terminal("a"), terminal("b")], [info("t-a"), info("t-b")],
    );

    connected.rail.select("a");
    emulators.get("t-a")!.selectedText = "copy";
    clip.writeText.mockRejectedValueOnce(new Error("copy denied"));
    key(container.querySelector("[data-terminal='t-a']")!, "c", { metaKey: true });
    expect(await screen.findByRole("alert", { name: "copy denied" })).not.toBeNull();

    connected.rail.select("b");
    expect(screen.queryByRole("alert", { name: "copy denied" })).toBeNull();
    connected.rail.select("a");
    expect(await screen.findByRole("alert", { name: "copy denied" })).not.toBeNull();
    expect(screen.getByRole("alert").querySelector('svg.lucide-x[aria-hidden="true"]')).not.toBeNull();
  });
});
