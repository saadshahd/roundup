import { describe, expect, it } from "vitest";
import { adjacentId } from "./keys";

describe("u31 rail keyboard: adjacentId", () => {
  it("u31_moves_to_the_next_id_going_down", () => {
    expect(adjacentId(["a", "b", "c"], "a", 1)).toBe("b");
  });

  it("u31_moves_to_the_previous_id_going_up", () => {
    expect(adjacentId(["a", "b", "c"], "b", -1)).toBe("a");
  });

  it("u31_does_not_wrap_past_the_last_id", () => {
    expect(adjacentId(["a", "b", "c"], "c", 1)).toBe("c");
  });

  it("u31_does_not_wrap_past_the_first_id", () => {
    expect(adjacentId(["a", "b", "c"], "a", -1)).toBe("a");
  });
});
