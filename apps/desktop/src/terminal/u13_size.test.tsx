import { cleanup } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ITerminalAddon } from "@xterm/xterm";
import { attachRenderer, createXtermEmulators } from "./emulator";
import type { RendererAddon } from "./emulator";
import { callsTo, mountPane } from "./paneHarness";
import { event, info, node, terminal } from "../testing/nodes";

const nextFrame = () => vi.advanceTimersToNextFrame();

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] });
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("u13 size", () => {
  it("u13_a_terminal_first_shown_is_resized_to_the_emulators_fitted_size", async () => {
    const { app, connected } = await mountPane([node("a")], [info("t-a")]);

    connected.rail.select("a");

    expect(callsTo(app, "terminal.resize")).toEqual([{ id: "t-a", cols: 100, rows: 30 }]);
  });

  it("u13_showing_a_terminal_again_resizes_it_to_the_current_fit", async () => {
    const { app, connected, emulators } = await mountPane([node("a"), node("b")], [info("t-a"), info("t-b")]);

    connected.rail.select("a");
    connected.rail.select("b");

    const first = emulators.get("t-a");

    if (first) first.size = { cols: 90, rows: 20 };

    connected.rail.select("a");

    expect(callsTo(app, "terminal.resize").at(-1)).toEqual({ id: "t-a", cols: 90, rows: 20 });
  });

  it("u13_a_window_resize_refits_the_shown_terminal", async () => {
    const { app, connected, emulators } = await mountPane([node("a")], [info("t-a")]);

    connected.rail.select("a");

    const shown = emulators.get("t-a");

    if (shown) shown.size = { cols: 120, rows: 40 };

    window.dispatchEvent(new Event("resize"));
    nextFrame();

    expect(callsTo(app, "terminal.resize")).toEqual([
      { id: "t-a", cols: 100, rows: 30 },
      { id: "t-a", cols: 120, rows: 40 },
    ]);
  });

  it("u13_a_burst_of_window_resizes_calls_resize_once_per_animation_frame", async () => {
    const { app, connected } = await mountPane([node("a")], [info("t-a")]);

    connected.rail.select("a");
    window.dispatchEvent(new Event("resize"));
    window.dispatchEvent(new Event("resize"));
    window.dispatchEvent(new Event("resize"));
    nextFrame();

    expect(callsTo(app, "terminal.resize")).toHaveLength(2);
  });

  it("u13_opening_a_drawer_never_resizes_the_terminal", async () => {
    const { app, connected } = await mountPane([node("a")], [info("t-a")]);

    connected.rail.select("a");
    connected.drawer.open(() => <p>detail</p>);
    nextFrame();

    expect(callsTo(app, "terminal.resize")).toHaveLength(1);
  });

  it("u13_an_exited_terminal_is_not_resized", async () => {
    const { app, connected } = await mountPane([terminal("a")], [info("t-a", { running: false, exit_code: 0 })]);

    connected.rail.select("a");
    window.dispatchEvent(new Event("resize"));
    nextFrame();

    expect(callsTo(app, "terminal.resize")).toEqual([]);
  });

  it("u13_a_terminal_that_exits_is_no_longer_resized", async () => {
    const { app, connected } = await mountPane([terminal("a")], [info("t-a")]);

    connected.rail.select("a");
    app.emit(event({ name: "terminal.exited", data: { id: "t-a", code: 0 } }));
    window.dispatchEvent(new Event("resize"));
    nextFrame();

    expect(callsTo(app, "terminal.resize")).toHaveLength(1);
  });

  it("u13_no_resize_is_scheduled_with_nothing_selected", async () => {
    const { app } = await mountPane([node("a")], [info("t-a")]);

    window.dispatchEvent(new Event("resize"));
    nextFrame();

    expect(callsTo(app, "terminal.resize")).toEqual([]);
  });
});

const addon = (loadContext: () => void) => {
  const created = { lost: () => {}, disposed: false };

  return {
    created,
    create: () => {
      loadContext();

      return {
        activate: () => {},
        dispose: () => {
          created.disposed = true;
        },
        onContextLoss: (listener) => {
          created.lost = listener;

          return { dispose: () => {} };
        },
      } satisfies RendererAddon;
    },
  };
};

describe("u13 renderer", () => {
  it("u13_webgl_is_loaded_when_the_webview_grants_a_context", () => {
    const loaded: ITerminalAddon[] = [];
    const warnings: string[] = [];
    const webgl = addon(() => {});

    const used = attachRenderer({ loadAddon: (loading) => loaded.push(loading) }, webgl.create, (message) => warnings.push(message));

    expect([used, loaded.length, warnings]).toEqual([true, 1, []]);
  });

  it("u13_the_dom_renderer_is_used_with_one_warning_when_the_webview_grants_no_context", () => {
    const warnings: string[] = [];

    const webgl = addon(() => {
      throw new Error("WebGL2 not supported");
    });

    const used = attachRenderer({ loadAddon: () => {} }, webgl.create, (message) => warnings.push(message));

    expect([used, warnings]).toEqual([
      false,
      ["WebGL renderer unavailable, using the DOM renderer: WebGL2 not supported"],
    ]);
  });

  it("u13_a_lost_webgl_context_disposes_the_addon", () => {
    const webgl = addon(() => {});

    attachRenderer({ loadAddon: () => {} }, webgl.create, () => {});
    webgl.created.lost();

    expect(webgl.created.disposed).toBe(true);
  });

  it("u13_the_warning_for_a_refused_webgl_context_is_logged_once_across_terminals", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const noMedia = { matches: false, addListener: () => {}, removeListener: () => {}, addEventListener: () => {}, removeEventListener: () => {} };

    vi.stubGlobal("matchMedia", () => noMedia);

    const create = createXtermEmulators();

    create("t-a").show(document.createElement("div"));
    create("t-b").show(document.createElement("div"));

    const warnings = warn.mock.calls.length;

    warn.mockRestore();
    vi.unstubAllGlobals();

    expect(warnings).toBe(1);
  });
});
