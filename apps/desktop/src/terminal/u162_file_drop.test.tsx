import { cleanup } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DaemonExit, FileDrop } from "../app/seam";
import { tauriFileDrop } from "../app/tauri";
import { RpcError } from "../app/seam";
import { info, terminal } from "../testing/nodes";
import { callsTo, mountPane } from "./paneHarness";

afterEach(cleanup);

const INSIDE = { x: 50, y: 50 };

const OUTSIDE = { x: 500, y: 500 };

const drop = (paths: string[], at = INSIDE): FileDrop => ({ phase: "drop", paths, ...at });

const over = (at = INSIDE): FileDrop => ({ phase: "over", paths: [], ...at });

const mount = async (daemonExit?: () => DaemonExit | null, terminals = [info("t-a")]) => {
  const mounted = await mountPane([terminal("a"), terminal("b")], [...terminals, info("t-b")], 0, daemonExit);

  mounted.connected.rail.select("a");

  const body = mounted.container.querySelector<HTMLElement>(".pane-body")!;

  body.getBoundingClientRect = () => new DOMRect(0, 0, 100, 100);
  await Promise.resolve();

  return { ...mounted, body };
};

describe("u162 drop files on a pane", () => {
  it("u162_a_dropped_path_is_quoted_and_pasted_once", async () => {
    const { app, emulators } = await mount();

    app.dropFiles(drop(["/p/it's a file.txt"]));

    expect(emulators.get("t-a")!.pasted).toEqual([`'/p/it'\\''s a file.txt' `]);
    await vi.waitFor(() => expect(callsTo(app, "terminal.write")).toHaveLength(1));
  });

  it("u162_three_paths_go_in_one_line_with_no_newline", async () => {
    const { app, emulators } = await mount();

    app.dropFiles(drop(["/a", "/b", "/c"]));

    expect(emulators.get("t-a")!.pasted).toEqual(["'/a' '/b' '/c' "]);
  });

  it("u162_a_directory_is_quoted_like_a_file", async () => {
    const { app, emulators } = await mount();

    app.dropFiles(drop(["/p/some dir"]));

    expect(emulators.get("t-a")!.pasted).toEqual(["'/p/some dir' "]);
  });

  it("u162_two_drops_are_two_pastes", async () => {
    const { app, emulators } = await mount();

    app.dropFiles(drop(["/a"]));
    app.dropFiles(drop(["/b"]));

    expect(emulators.get("t-a")!.pasted).toEqual(["'/a' ", "'/b' "]);
  });

  it("u162_the_ring_shows_on_over_inside_and_not_outside", async () => {
    const { app, body } = await mount();

    app.dropFiles(over(OUTSIDE));
    expect(body.classList.contains("drop-over")).toBe(false);

    app.dropFiles(over());
    expect(body.classList.contains("drop-over")).toBe(true);
  });

  it("u162_leave_a_drop_and_a_pane_switch_clear_the_ring", async () => {
    const { app, body, connected } = await mount();

    app.dropFiles(over());
    app.dropFiles({ phase: "leave", paths: [], x: 0, y: 0 });
    expect(body.classList.contains("drop-over")).toBe(false);

    app.dropFiles(over());
    app.dropFiles(drop(["/a"]));
    expect(body.classList.contains("drop-over")).toBe(false);

    app.dropFiles(over());
    connected.rail.select("b");
    await Promise.resolve();
    expect(body.classList.contains("drop-over")).toBe(false);
  });

  it("u162_a_drop_outside_the_pane_does_nothing", async () => {
    const { app, emulators } = await mount();

    app.dropFiles(drop(["/a"], OUTSIDE));

    expect(emulators.get("t-a")!.pasted).toEqual([]);
  });

  it("u162_a_drop_on_an_open_drawer_over_the_pane_does_nothing", async () => {
    const { app, emulators, body } = await mount();
    const drawer = document.createElement("aside");

    document.body.append(drawer);
    document.elementFromPoint = () => drawer;

    try {
      app.dropFiles(over());
      expect(body.classList.contains("drop-over")).toBe(false);

      app.dropFiles(drop(["/a"]));
      expect(emulators.get("t-a")!.pasted).toEqual([]);

      document.elementFromPoint = () => body;
      app.dropFiles(drop(["/a"]));
      expect(emulators.get("t-a")!.pasted).toEqual(["'/a' "]);
    } finally {
      // SAFETY: jsdom defines no elementFromPoint, so removing the stub restores the document.
      delete (document as { elementFromPoint?: unknown }).elementFromPoint;
      drawer.remove();
    }
  });

  it("u162_an_exited_terminal_takes_nothing", async () => {
    const { app, emulators, body } = await mount(undefined, [info("t-a", { running: false, exit_code: 0 })]);

    app.dropFiles(over());
    expect(body.classList.contains("drop-over")).toBe(false);
    app.dropFiles(drop(["/a"]));
    expect(emulators.get("t-a")!.pasted).toEqual([]);
  });

  it("u162_daemon_exited_takes_nothing", async () => {
    const [exit, setExit] = createSignal<DaemonExit | null>(null);
    const { app, emulators, body } = await mount(exit);

    setExit({ code: 1 });
    app.dropFiles(over());
    expect(body.classList.contains("drop-over")).toBe(false);
    app.dropFiles(drop(["/a"]));
    expect(emulators.get("t-a")!.pasted).toEqual([]);
  });

  it("u162_an_empty_drop_calls_nothing", async () => {
    const { app, emulators } = await mount();

    app.dropFiles(drop([]));

    expect(emulators.get("t-a")!.pasted).toEqual([]);
    expect(callsTo(app, "terminal.write")).toEqual([]);
  });

  it("u162_a_failed_write_shows_its_message", async () => {
    const { app, container } = await mount();

    app.handlers["terminal.write"] = () => {
      throw new RpcError(-32000, "write failed");
    };

    app.dropFiles(drop(["/a"]));

    await vi.waitFor(() => expect(container.querySelector(".pane-failure")!.textContent).toContain("write failed"));
  });

  it("u162_the_ring_moves_no_row_and_no_size", async () => {
    const { app, body, emulators } = await mount();
    const screen = body.querySelector(".pane-screen")!;

    const before = [body.getBoundingClientRect(), screen.getBoundingClientRect()];
    const fits = emulators.get("t-a")!.fits;

    app.dropFiles(over());
    expect([body.getBoundingClientRect(), screen.getBoundingClientRect()]).toEqual(before);
    app.dropFiles({ phase: "leave", paths: [], x: 0, y: 0 });
    expect([body.getBoundingClientRect(), screen.getBoundingClientRect()]).toEqual(before);
    expect(emulators.get("t-a")!.fits).toBe(fits);
  });

  it("u162_focus_returns_to_the_pane_after_a_drop", async () => {
    const { app, emulators } = await mount();
    const emulator = emulators.get("t-a")!;
    const focus = vi.spyOn(emulator, "focus");

    app.dropFiles(drop(["/a"]));

    expect(focus).toHaveBeenCalledTimes(1);
  });

  it("u162_the_tauri_adapter_converts_a_physical_drop_to_css_pixels", async () => {
    const heard: FileDrop[] = [];
    let deliver: (event: { payload: unknown }) => void = () => {};

    const webview = {
      onDragDropEvent: async (handler: typeof deliver) => {
        deliver = handler;

        return () => {};
      },
    };

    // SAFETY: the stub implements the one method the adapter calls.
    await tauriFileDrop(() => webview as never, () => 2)((drop) => heard.push(drop));
    deliver({ payload: { type: "drop", paths: ["/a"], position: { x: 200, y: 100 } } });
    deliver({ payload: { type: "enter", paths: ["/a"], position: { x: 20, y: 10 } } });
    deliver({ payload: { type: "leave" } });

    expect(heard).toEqual([
      { phase: "drop", paths: ["/a"], x: 100, y: 50 },
      { phase: "over", paths: [], x: 10, y: 5 },
      { phase: "leave", paths: [], x: 0, y: 0 },
    ]);
  });
});
