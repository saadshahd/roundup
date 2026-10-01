import { cleanup, screen } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import { afterEach, describe, expect, it } from "vitest";
import type { DaemonExit } from "../app/seam";
import { callsTo, info, mountPane, node } from "./paneHarness";

afterEach(cleanup);

const mountGone = async () => {
  const [exit, setExit] = createSignal<DaemonExit | null>(null);
  const mounted = await mountPane([node("a")], [info("t-a")], 0, exit);

  mounted.connected.rail.select("a");

  return { ...mounted, setExit };
};

describe("u25 the Daemon is gone", () => {
  it("u25_typing_sends_no_terminal_write", async () => {
    const { app, emulators, setExit } = await mountGone();

    setExit({ code: 1 });
    emulators.get("t-a")?.type(Uint8Array.of(108));
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(callsTo(app, "terminal.write")).toEqual([]);
  });

  it("u25_stop_is_disabled", async () => {
    const { setExit } = await mountGone();

    const stop = screen.getByRole("button", { name: "stop" });

    expect(stop.hasAttribute("disabled")).toBe(false);
    setExit({ code: 1 });

    expect(stop.hasAttribute("disabled")).toBe(true);
  });

  it("u25_a_window_resize_sends_no_terminal_resize", async () => {
    const { app, setExit } = await mountGone();
    const before = callsTo(app, "terminal.resize").length;

    setExit({ code: 1 });
    window.dispatchEvent(new Event("resize"));
    await new Promise((resolve) => requestAnimationFrame(resolve));

    expect(callsTo(app, "terminal.resize")).toHaveLength(before);
  });
});
