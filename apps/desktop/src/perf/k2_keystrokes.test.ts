import { afterEach, describe, expect, it } from "vitest";
import { reportTerminal } from "./run";
import { createKeystrokeProbe, typeKeys } from "./keystrokes";

afterEach(() => document.body.replaceChildren());

const bytes = (text: string) => new TextEncoder().encode(text);

const clock = () => {
  let at = 0;

  return { now: () => at, advance: (ms: number) => void (at += ms) };
};

describe("k2 a sample is keydown to the render that shows its echo", () => {
  it("r20_another_terminal_cannot_supply_the_echo_or_finish_its_render", () => {
    const time = clock();
    const probe = createKeystrokeProbe(time.now, "a", () => "shell");

    void probe.keydown();
    time.advance(1);
    probe.input("shell");
    probe.dispatched("shell");
    time.advance(2);
    probe.arrived(bytes("a"), "agent");
    probe.written(bytes("a"), "agent");
    probe.rendered("agent");

    expect(probe.summary(0)).toMatchObject({ n: 0, p95: null });
    time.advance(4);
    probe.arrived(bytes("a"), "shell");
    probe.written(bytes("a"), "shell");
    time.advance(1);
    probe.rendered("agent");

    expect(probe.summary(0)).toMatchObject({ n: 0, p95: null });
    time.advance(12);
    probe.rendered("shell");

    expect(probe.summary(0)).toMatchObject({ n: 1, dropped: 0, p95: 20 });
    expect(probe.hops(0)).toMatchObject({
      dispatch_to_echo_event: { n: 1, p95: 6 },
      parsed_to_frame: { n: 1, p95: 13 },
    });
  });

  it("r20_another_terminals_write_cannot_stamp_the_measured_input", () => {
    const time = clock();
    const probe = createKeystrokeProbe(time.now, "a", () => "shell");

    void probe.keydown();
    time.advance(1);
    probe.input("agent");
    time.advance(1);
    probe.input("shell");
    time.advance(1);
    probe.dispatched("agent");
    time.advance(2);
    probe.dispatched("shell");
    time.advance(1);
    probe.replied("agent");
    time.advance(3);
    probe.replied("shell");
    probe.arrived(bytes("a"), "shell");
    probe.written(bytes("a"), "shell");
    probe.rendered("shell");

    expect(probe.hops(0)).toMatchObject({
      keydown_to_input: { n: 1, p95: 2 },
      input_to_dispatch: { n: 1, p95: 3 },
      write_reply: { n: 1, p95: 4 },
    });
  });

  it("r20_keys_reach_the_terminal_that_printed_ready_even_with_an_earlier_agent_input", async () => {
    document.body.innerHTML = `<div class="bubble" data-bubble="door"><textarea class="xterm-helper-textarea"></textarea></div>
      <div class="bubble" data-bubble="shell"><textarea class="xterm-helper-textarea"></textarea></div>`;
    const run = reportTerminal(() => ({ write: (_bytes: Uint8Array, parsed?: () => void) => parsed?.() }));
    const door = run.createEmulator("door");
    const shell = run.createEmulator("shell");
    const received: string[] = [];

    for (const input of document.querySelectorAll("textarea")) input.addEventListener("keypress", () => received.push(input.closest<HTMLElement>("[data-bubble]")!.dataset.bubble!));

    expect(run.input()).toBeNull();
    shell.write(bytes("ready\n"));
    door.write(bytes("welcome"));
    await run.printed;
    run.input()!.dispatchEvent(new KeyboardEvent("keypress", { key: "a" }));

    expect(received).toEqual(["shell"]);
  });

  it("r20_the_ready_line_survives_every_output_split_after_parsing", async () => {
    for (const line of ["ready\n", "ready\r\n"]) {
      for (let split = 1; split < line.length; split += 1) {
        const parsed: (() => void)[] = [];
        const run = reportTerminal(() => ({ write: (_bytes: Uint8Array, done?: () => void) => { if (done) parsed.push(done); } }));
        const shell = run.createEmulator("shell");
        const agent = run.createEmulator("agent");

        shell.write(bytes(line.slice(0, split)));
        parsed.shift()!();
        agent.write(bytes("unrelated output\n"));
        parsed.shift()!();
        shell.write(bytes(line.slice(split)));

        expect(run.target()).toBeNull();
        parsed.shift()!();
        expect(run.target()).toBe("shell");
        await run.printed;
      }
    }
  });

  it("r20_only_a_complete_ready_line_selects_the_report_terminal", async () => {
    const run = reportTerminal(() => ({ write: (_bytes: Uint8Array, parsed?: () => void) => parsed?.() }));
    const agent = run.createEmulator("agent");
    const shell = run.createEmulator("shell");

    for (const text of ["already\n", "not ready\n", "readying\n", "x".repeat(100_000) + "rea", "dy\n", "ready"]) {
      agent.write(bytes(text));
      expect(run.target()).toBeNull();
    }

    shell.write(bytes("welcome\r\nready\n"));
    await run.printed;
    expect(run.target()).toBe("shell");
  });

  it("k2_a_key_with_no_rendered_echo_in_time_is_dropped_and_counted", async () => {
    const time = clock();
    const probe = createKeystrokeProbe(time.now, "a", () => "shell");
    const sleeps: number[] = [];

    await typeKeys(
      probe,
      { press: () => {}, sleep: async (ms) => void sleeps.push(ms), random: () => 0.5, expired: () => false },
      1,
    );

    expect(sleeps).toEqual([500, 30]);

    void probe.keydown();
    probe.written(bytes("a"), "shell");
    time.advance(4);
    probe.rendered("shell");

    expect(probe.summary(0)).toMatchObject({ n: 1, dropped: 1, p50: 4 });
  });

  it("k2_the_first_samples_are_skipped_and_the_rest_summarised_by_nearest_rank", () => {
    const time = clock();
    const probe = createKeystrokeProbe(time.now, "a", () => "shell");

    for (const wait of [100, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]) {
      void probe.keydown();
      probe.written(bytes("a"), "shell");
      time.advance(wait);
      probe.rendered("shell");
    }

    expect(probe.summary(1)).toEqual({ n: 20, dropped: 0, p50: 10, p95: 19, p99: 20, max: 20 });
  });

  it("r21_dropped_keys_cannot_be_hidden_by_warmup", async () => {
    const time = clock();
    const probe = createKeystrokeProbe(time.now, "a", () => "shell");
    const sleep = async () => {};

    await typeKeys(probe, { press: () => {}, sleep, random: () => 0, expired: () => false }, 1);

    void probe.keydown();
    probe.written(bytes("a"), "shell");
    time.advance(6);
    probe.rendered("shell");
    await typeKeys(probe, { press: () => {}, sleep, random: () => 0, expired: () => false }, 1);

    expect(probe.summary(1)).toMatchObject({ n: 1, dropped: 2, p50: 6 });
  });

  it("r20_hop_report_accounts_for_the_echo_round_trip", () => {
    const time = clock();
    const probe = createKeystrokeProbe(time.now, "a", () => "shell");

    // A warm-up key (skipped), then a measured key whose output is preceded by unrelated output and a render before the echo.
    for (const queued of [0, 7]) {
      void probe.keydown();
      time.advance(2);
      probe.input("shell");
      time.advance(queued);
      probe.encoded("agent");
      time.advance(2);
      probe.encoded("shell");
      time.advance(1);
      probe.dispatched("shell");
      time.advance(1);
      probe.arrived(bytes("noise"), "shell");
      probe.written(bytes("noise"), "shell");
      probe.rendered("shell");
      time.advance(3);
      probe.arrived(bytes("a"), "shell");
      probe.replied("shell");
      time.advance(4);
      probe.written(bytes("a"), "shell");
      time.advance(5);
      probe.rendered("shell");
    }

    expect(probe.hops(1)).toEqual({
      keydown_to_input: { n: 1, p50: 2, p95: 2 },
      input_to_dispatch: { n: 1, p50: 10, p95: 10 },
      input_to_encoded: { n: 1, p50: 9, p95: 9 },
      encoded_to_dispatch: { n: 1, p50: 1, p95: 1 },
      dispatch_to_echo_event: { n: 1, p50: 4, p95: 4 },
      echo_event_to_parsed: { n: 1, p50: 4, p95: 4 },
      parsed_to_frame: { n: 1, p50: 5, p95: 5 },
      write_reply: { n: 1, p50: 4, p95: 4 },
    });
    expect(probe.summary(1)).toMatchObject({ n: 1, p50: 25 });
  });

  it("r20_a_run_without_measured_echoes_retains_its_failure_counts_in_json", async () => {
    const probe = createKeystrokeProbe(() => 0, "a", () => "shell");
    let typed = 0;

    await typeKeys(probe, {
      press: () => void (typed += 1), sleep: async () => {}, random: () => 0,
      expired: () => typed >= 2,
    }, 5);

    expect(JSON.parse(JSON.stringify(probe.summary(1)))).toEqual({
      error: "no key was echoed and rendered after the warm-up",
      n: 0, dropped: 5, p50: null, p95: null, p99: null, max: null,
    });
  });

  it("r20_a_run_out_of_time_counts_the_untyped_keys_as_dropped_and_still_reports", async () => {
    const time = clock();
    const probe = createKeystrokeProbe(time.now, "a", () => "shell");
    let typed = 0;

    await typeKeys(
      probe,
      { press: () => void (typed += 1), sleep: async () => {}, random: () => 0, expired: () => typed >= 2 },
      5,
    );

    void probe.keydown();
    probe.written(bytes("a"), "shell");
    time.advance(4);
    probe.rendered("shell");

    // Two keys were typed and dropped, three never typed.
    expect(typed).toBe(2);
    expect(probe.summary(0)).toMatchObject({ n: 1, dropped: 5, p50: 4 });
  });
});
