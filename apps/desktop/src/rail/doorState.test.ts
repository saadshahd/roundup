import { describe, expect, it } from "vitest";
import { door, workstream } from "../testing/nodes";
import { doorStateOf, doorStateText } from "./doorState";

const none = { kind: "unknown" } as const;

describe("u143 Door state", () => {
  it("u143_a_running_Door_has_no_state", () => {
    expect(doorStateOf(door("r", "working", "w"), null, false, false)).toBeNull();
  });

  it("u143_a_pending_start_is_starting_even_after_an_earlier_failure", () => {
    expect(doorStateOf(workstream("r"), none, true, true)).toEqual({ kind: "starting" });
  });

  it("u143_a_recorded_start_failure_is_failed_not_stopped", () => {
    expect(doorStateOf(door("r", "done", "d", { terminal_id: null }), none, false, true)).toEqual({ kind: "failed" });
  });

  it("u143_a_Workstream_never_launched_is_not_started", () => {
    expect(doorStateOf(workstream("r"), none, false, false)).toEqual({ kind: "never-started" });
  });

  it("u143_a_Workstream_launched_before_is_stopped_with_how_it_ended", () => {
    expect(doorStateOf(door("r", "error", "e"), { kind: "code", code: 137 }, false, false)).toEqual({ kind: "stopped", exit: { kind: "code", code: 137 } });
  });

  it("u143_the_words_tell_the_four_states_apart", () => {
    expect([
      doorStateText({ kind: "starting" }),
      doorStateText({ kind: "failed" }),
      doorStateText({ kind: "never-started" }),
      doorStateText({ kind: "stopped", exit: none }),
      doorStateText({ kind: "stopped", exit: { kind: "code", code: 137 } }),
      doorStateText({ kind: "stopped", exit: { kind: "signal" } }),
    ]).toEqual([
      "starting Door…",
      "Door did not start",
      "Door not started",
      "Door stopped",
      "Door stopped, exited 137",
      "Door stopped, exited by signal",
    ]);
  });
});
