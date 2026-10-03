import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as xtermModule from "@xterm/xterm";
import { Terminal } from "@xterm/xterm";
import type { Event as DaemonEvent } from "@contracts/Event";
import { ConnectedProjectContext } from "../state/connectedProject";
import type { FakeApp } from "../testing/fakeApp";
import { event, info, node, agent } from "../testing/nodes";
import { connectFakeProject } from "./paneHarness";
import { Pane } from "./Pane";
import { toBase64 } from "./base64";
import { createXtermEmulators } from "./emulator";
import type { Emulator, EmulatorFactory, Size } from "./emulator";

import type { Snapshot } from "@contracts/terminal/Snapshot";

type Reply = Snapshot | Error;

const snapshotOf = (text: string, after: number, size: Size = { cols: 100, rows: 30 }): Snapshot => ({
  ...size,
  after,
  data: toBase64(new TextEncoder().encode(text)),
});

const chunk = (id: string, text: string, offset: number): DaemonEvent =>
  event({ name: "terminal.output", data: { id, data: toBase64(new TextEncoder().encode(text)), offset } });

type Recorder = Emulator & { log: string[]; shown: boolean; fits: number };

/** Records, in order, `size <cols>x<rows>` and each written text. */
const recordingEmulators = () => {
  const made = new Map<string, Recorder>();

  const factory: EmulatorFactory = (id) => {
    const log: string[] = [];

    const emulator: Recorder = {
      log,
      shown: false,
      fits: 0,
      setSize: (size) => log.push(`size ${size.cols}x${size.rows}`),
      reset: () => log.push("reset"),
      write: (bytes, parsed) => {
        log.push(new TextDecoder().decode(bytes));
        parsed?.();
      },
      onInput: () => {},
      onScroll: () => {},
      show: () => {
        emulator.shown = true;

        return { cols: 100, rows: 30 };
      },
      fit: () => {
        emulator.fits += 1;

        return { cols: 100, rows: 30 };
      },
      focus: () => {},
      isAtBottom: () => true,
      scrollToBottom: () => {},
      dispose: () => {},
    };

    made.set(id, emulator);

    return emulator;
  };

  return { factory, made };
};

/** A Pane over a Daemon whose `terminal.snapshot` answers are controlled by the test, one per call. */
const mount = async (
  tree = [node("a")],
  terminals = [info("t-a")],
  configure?: (app: FakeApp) => void,
) => {
  const { app, connected } = await connectFakeProject(tree, terminals);
  const { factory, made } = recordingEmulators();
  const pending: ((reply: Reply) => void)[] = [];

  app.handlers["terminal.snapshot"] = () =>
    new Promise<Snapshot>((resolve, reject) =>
      pending.push((reply) => (reply instanceof Error ? reject(reply) : resolve(reply))),
    );
  configure?.(app);

  render(() => (
    <ConnectedProjectContext.Provider value={connected}>
      <Pane createEmulator={factory} />
    </ConnectedProjectContext.Provider>
  ));
  connected.rail.select(tree[0]?.id ?? null);

  const snapshots = () => app.calls.filter((call) => call.method === "terminal.snapshot");
  const log = () => made.get("t-a")?.log ?? [];
  const logOf = (id: string) => made.get(id)?.log ?? [];

  return { app, connected, pending, snapshots, log, logOf, shown: () => made.get("t-a")?.shown ?? false, fits: () => made.get("t-a")?.fits ?? 0 };
};

const typed = (app: { calls: { method: string }[] }) => app.calls.filter((call) => call.method === "terminal.write");

afterEach(cleanup);

describe("u103 a reloaded pane shows the screen", () => {
  it("u103_the_real_emulator_resets_after_queued_output_before_the_replacement_screen", async () => {
    const spy = vi.spyOn(xtermModule, "Terminal");
    const emulator = createXtermEmulators()("t-a");
    // SAFETY: the factory created exactly one Terminal while the spy was attached.
    const terminal = spy.mock.instances[0] as Terminal;

    spy.mockRestore();
    emulator.setSize({ cols: 20, rows: 5 });
    emulator.write(new TextEncoder().encode("old"));
    emulator.reset();
    emulator.write(new TextEncoder().encode("new"));
    await new Promise<void>((resolve) => terminal.write("", resolve));

    expect(terminal.buffer.active.getLine(0)?.translateToString().trim()).toBe("new");
    emulator.dispose();
  });

  it("u103_the_snapshot_is_asked_for_before_any_output_reaches_the_emulator", async () => {
    const { app, pending, snapshots, log } = await mount();

    app.emit(chunk("t-a", "live", 0));

    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    expect(snapshots()[0]?.params).toEqual({ id: "t-a" });
    expect(log()).toEqual([]);
    pending[0]?.(snapshotOf("screen", 0));
    await vi.waitFor(() => expect(log()).toContain("live"));
  });

  it("u103_a_snapshot_that_arrives_after_the_pane_was_shown_refits_the_emulator", async () => {
    const { pending, snapshots, shown, fits } = await mount();

    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    expect(shown()).toBe(true);
    pending[0]?.(snapshotOf("screen", 0, { cols: 120, rows: 40 }));

    await vi.waitFor(() => expect(fits()).toBe(1));
  });

  it("u103_the_emulator_is_sized_then_gets_the_data_then_the_held_and_later_output_in_order", async () => {
    const { app, pending, snapshots, log } = await mount();

    app.emit(chunk("t-a", "one", 6));
    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    app.emit(chunk("t-a", "two", 9));
    pending[0]?.(snapshotOf("screen", 6, { cols: 120, rows: 40 }));
    await vi.waitFor(() => expect(log()).toHaveLength(4));
    app.emit(chunk("t-a", "three", 12));

    expect(log()).toEqual(["size 120x40", "screen", "one", "two", "three"]);
  });

  it("u103_one_snapshot_call_serves_one_terminal_however_much_output_arrives", async () => {
    const { app, pending, snapshots } = await mount();

    for (let offset = 0; offset < 5; offset += 1) app.emit(chunk("t-a", "x", offset));
    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    pending[0]?.(snapshotOf("", 0));
    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
  });

  it("u103_output_held_during_a_slow_snapshot_stays_bounded_and_resnapshots_after_a_dropped_prefix", async () => {
    const { app, pending, snapshots } = await mount();
    const large = "x".repeat(400_000);

    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    app.emit(chunk("t-a", large, 0));
    app.emit(chunk("t-a", large, large.length));
    app.emit(chunk("t-a", large, large.length * 2));
    pending[0]?.(snapshotOf("", 0));

    await vi.waitFor(() => expect(snapshots()).toHaveLength(2));
    pending[1]?.(snapshotOf("complete", large.length * 3));
  });

  it("u103_an_event_at_or_below_the_snapshot_is_dropped_and_a_straddling_one_keeps_only_its_tail", async () => {
    const { app, pending, snapshots, log } = await mount();

    app.emit(chunk("t-a", "abcd", 0));
    app.emit(chunk("t-a", "efgh", 4));
    app.emit(chunk("t-a", "ijkl", 8));
    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    pending[0]?.(snapshotOf("SCREEN", 10));

    await vi.waitFor(() => expect(log()).toEqual(["size 100x30", "SCREEN", "kl"]));
  });

  it("u103_a_duplicate_range_after_the_call_is_written_once", async () => {
    const { app, pending, snapshots, log } = await mount();

    app.emit(chunk("t-a", "abc", 0));
    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    pending[0]?.(snapshotOf("S", 0));
    await vi.waitFor(() => expect(log()).toContain("abc"));
    app.emit(chunk("t-a", "abc", 0));
    app.emit(chunk("t-a", "def", 3));

    expect(log()).toEqual(["size 100x30", "S", "abc", "def"]);
  });

  it("u103_a_gap_requests_a_fresh_snapshot_before_any_later_output", async () => {
    const { app, pending, snapshots, log } = await mount();

    app.emit(chunk("t-a", "late", 50));
    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    pending[0]?.(snapshotOf("S", 10));
    await vi.waitFor(() => expect(snapshots()).toHaveLength(2));
    expect(log()).toEqual(["size 100x30", "S"]);
    pending[1]?.(snapshotOf("COMPLETE", 54));
    await vi.waitFor(() => expect(log()).toEqual(["size 100x30", "S", "reset", "size 100x30", "COMPLETE"]));
  });

  it("u103_a_failed_gap_recovery_preserves_the_last_complete_screen_and_shows_one_failure_line", async () => {
    const { app, pending, snapshots, log } = await mount();

    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    pending[0]?.(snapshotOf("complete", 8));
    await vi.waitFor(() => expect(log()).toEqual(["size 100x30", "complete"]));
    app.emit(chunk("t-a", "late", 50));
    await vi.waitFor(() => expect(snapshots()).toHaveLength(2));
    pending[1]?.(new Error("recovery failed"));

    expect((await screen.findByText("✕ terminal.snapshot: recovery failed")).tagName).toBe("P");
    expect(log()).toEqual(["size 100x30", "complete"]);
  });

  it("u103_output_overlapping_the_recovery_cut_writes_only_the_remaining_bytes", async () => {
    const { app, pending, snapshots, log } = await mount();

    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    pending[0]?.(snapshotOf("old", 8));
    await vi.waitFor(() => expect(log()).toEqual(["size 100x30", "old"]));
    app.emit(chunk("t-a", "abcdef", 50));
    await vi.waitFor(() => expect(snapshots()).toHaveLength(2));
    pending[1]?.(snapshotOf("new", 53));

    await vi.waitFor(() => expect(log()).toEqual(["size 100x30", "old", "reset", "size 100x30", "new", "def"]));
  });

  it("u103_the_pane_still_sends_its_size_to_the_daemon_as_before", async () => {
    const { app, pending, snapshots } = await mount();

    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    pending[0]?.(snapshotOf("S", 0));

    await vi.waitFor(() =>
      expect(app.calls.filter((call) => call.method === "terminal.resize")[0]?.params).toEqual({
        id: "t-a",
        cols: 100,
        rows: 30,
      }),
    );
  });

  it("u103_a_failed_snapshot_shows_one_failure_line_and_the_emulator_starts_blank", async () => {
    const { app, pending, snapshots, log } = await mount();

    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    pending[0]?.(new Error("boom"));

    expect((await screen.findByText(/^✕ terminal\.snapshot:/)).tagName).toBe("P");
    expect(log()).toEqual([]);
    app.emit(chunk("t-a", "live", 0));
    expect(log()).toEqual(["live"]);
  });

  it("u103_a_late_resize_success_cannot_clear_a_failed_snapshot_line", async () => {
    const resize = Promise.withResolvers<null>();

    const { pending, snapshots, log } = await mount(undefined, undefined, (app) => {
      app.handlers["terminal.resize"] = () => resize.promise;
    });

    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    pending[0]?.(new Error("snapshot failed"));
    await screen.findByText("✕ terminal.snapshot: snapshot failed");
    resize.resolve(null);
    await Promise.resolve();
    await Promise.resolve();

    expect(screen.getByText("✕ terminal.snapshot: snapshot failed")).toBeTruthy();
    expect(log()).toEqual([]);
  });

  it("u103_output_held_during_a_failed_initial_snapshot_continues_as_live_output", async () => {
    const { app, pending, snapshots, log } = await mount();

    app.emit(chunk("t-a", "live", 0));
    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    expect(log()).toEqual([]);
    pending[0]?.(new Error("boom"));

    await vi.waitFor(() => expect(log()).toEqual(["live"]));
    app.emit(chunk("t-a", "more", 4));
    expect(log()).toEqual(["live", "more"]);
  });

  it("u103_a_background_terminals_snapshot_cannot_clear_or_show_another_panes_failure", async () => {
    const { app, connected, pending, snapshots, logOf } = await mount(
      [node("a"), node("b")],
      [info("t-a"), info("t-b")],
    );

    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    pending[0]?.(new Error("A failed"));
    await screen.findByText("✕ terminal.snapshot: A failed");

    app.emit(chunk("t-b", "other", 0));
    await vi.waitFor(() => expect(snapshots()).toHaveLength(2));
    pending[1]?.(snapshotOf("B screen", 0));
    await vi.waitFor(() => expect(logOf("t-b")).toContain("B screen"));
    expect(screen.getByText("✕ terminal.snapshot: A failed")).toBeTruthy();

    app.emit(chunk("t-b", "late", 50));
    await vi.waitFor(() => expect(snapshots()).toHaveLength(3));
    pending[2]?.(new Error("B failed"));
    await vi.waitFor(() => expect(screen.getByText("✕ terminal.snapshot: A failed")).toBeTruthy());
    expect(screen.queryByText("✕ terminal.snapshot: B failed")).toBeNull();

    connected.rail.select("b");
    expect(await screen.findByText("✕ terminal.snapshot: B failed")).toBeTruthy();
  });

  it("u103_nothing_is_typed_for_a_done_agent_and_its_screen_is_still_restored", async () => {
    const { app, pending, snapshots, log } = await mount([agent("a", "done", "finished")]);

    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    pending[0]?.(snapshotOf("last screen", 0));

    await vi.waitFor(() => expect(log()).toEqual(["size 100x30", "last screen"]));
    expect(typed(app)).toEqual([]);
  });

  it("u103_nothing_is_typed_for_an_exited_terminal_and_its_screen_is_still_restored", async () => {
    const { app, pending, snapshots, log } = await mount();
    app.emit(event({ name: "terminal.exited", data: { id: "t-a", code: 0 } }));

    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    pending[0]?.(snapshotOf("bye", 0));

    await vi.waitFor(() => expect(log()).toEqual(["size 100x30", "bye"]));
    expect(typed(app)).toEqual([]);
  });

  it("u103_a_takeover_restores_the_screen_without_typing", async () => {
    const { app, pending, snapshots, log } = await mount();

    app.emit(event({ name: "takeover.changed", data: { agent: "a", on: true } }));
    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    pending[0]?.(snapshotOf("user screen", 0));

    await vi.waitFor(() => expect(log()).toEqual(["size 100x30", "user screen"]));
    expect(typed(app)).toEqual([]);
  });
});
