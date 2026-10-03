import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Event as DaemonEvent } from "@contracts/Event";
import type { OutputEvent } from "@contracts/terminal/OutputEvent";
import { ConnectedProjectContext } from "../state/connectedProject";
import { event, info, node, agent } from "../testing/nodes";
import { connectFakeProject } from "./paneHarness";
import { Pane } from "./Pane";
import { toBase64 } from "./base64";
import type { Emulator, EmulatorFactory, Size } from "./emulator";

// Written by an architect before the Builder; the Builder makes these pass and never edits the
// test bodies. Everything the Builder may replace once the U104 types are generated is in the
// STUB block: the snapshot handler's types, `OutputEvent.offset`, and the fake emulator.
//
// Pinned seam: an Emulator gains `setSize(size: Size)`, called once, before the first `write`,
// with the snapshot's `cols` and `rows`. The factory stays `(id) => Emulator`: the emulator
// exists at once, but nothing is written to it until the snapshot call has answered.

// ---- STUB: replace with the generated types and the shared harness after U104 lands ----
type Snapshot = { cols: number; rows: number; after: number; data: string };

type Reply = Snapshot | Error;

const snapshotOf = (text: string, after: number, size: Size = { cols: 100, rows: 30 }): Snapshot => ({
  ...size,
  after,
  data: toBase64(new TextEncoder().encode(text)),
});

const chunk = (id: string, text: string, offset: number): DaemonEvent =>
  event({ name: "terminal.output", data: { id, data: toBase64(new TextEncoder().encode(text)), offset } as OutputEvent });

type Recorder = Emulator & { setSize(size: Size): void; log: string[] };

/** Records, in order, `size <cols>x<rows>` and each written text. */
const recordingEmulators = () => {
  const made = new Map<string, Recorder>();
  const factory: EmulatorFactory = (id) => {
    const log: string[] = [];
    const emulator: Recorder = {
      log,
      setSize: (size) => log.push(`size ${size.cols}x${size.rows}`),
      write: (bytes) => log.push(new TextDecoder().decode(bytes)),
      onInput: () => {},
      onScroll: () => {},
      show: () => ({ cols: 100, rows: 30 }),
      fit: () => ({ cols: 100, rows: 30 }),
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
// ---- END STUB ----

/** A Pane over a Daemon whose `terminal.snapshot` answers are controlled by the test, one per call. */
const mount = async (tree = [node("a")], terminals = [info("t-a")]) => {
  const { app, connected } = await connectFakeProject(tree, terminals);
  const { factory, made } = recordingEmulators();
  const pending: ((reply: Reply) => void)[] = [];

  (app.handlers as Record<string, unknown>)["terminal.snapshot"] = () =>
    new Promise<Snapshot>((resolve, reject) =>
      pending.push((reply) => (reply instanceof Error ? reject(reply) : resolve(reply))),
    );

  render(() => (
    <ConnectedProjectContext.Provider value={connected}>
      <Pane createEmulator={factory} />
    </ConnectedProjectContext.Provider>
  ));
  connected.rail.select(tree[0]?.id ?? null);

  const snapshots = () => app.calls.filter((call) => (call.method as string) === "terminal.snapshot");
  const log = () => made.get("t-a")?.log ?? [];

  return { app, pending, snapshots, log };
};

const typed = (app: { calls: { method: string }[] }) => app.calls.filter((call) => call.method === "terminal.write");

afterEach(cleanup);

describe("u103 a reloaded pane shows the screen", () => {
  it("u103_the_snapshot_is_asked_for_before_any_output_reaches_the_emulator", async () => {
    const { app, pending, snapshots, log } = await mount();

    app.emit(chunk("t-a", "live", 0));

    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    expect(snapshots()[0]?.params).toEqual({ id: "t-a" });
    expect(log()).toEqual([]);
    pending[0]?.(snapshotOf("screen", 0));
    await vi.waitFor(() => expect(log()).toContain("live"));
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

  it("u103_a_gap_between_the_snapshot_and_the_next_event_loses_nothing", async () => {
    const { app, pending, snapshots, log } = await mount();

    app.emit(chunk("t-a", "late", 50));
    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    pending[0]?.(snapshotOf("S", 10));

    await vi.waitFor(() => expect(log()).toEqual(["size 100x30", "S", "late"]));
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

    app.emit(chunk("t-a", "live", 0));
    await vi.waitFor(() => expect(snapshots()).toHaveLength(1));
    pending[0]?.(new Error("boom"));

    expect((await screen.findByText(/^✕ terminal\.snapshot:/)).tagName).toBe("P");
    await vi.waitFor(() => expect(log()).toEqual(["live"]));
    app.emit(chunk("t-a", "more", 4));
    expect(log()).toEqual(["live", "more"]);
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
});
