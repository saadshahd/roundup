import { cleanup, fireEvent } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { exitedTerminal, mountRail, rowOf } from "./railFixture";
import { agent, MINUTE, NOW, terminal } from "../testing/nodes";

afterEach(cleanup);

describe("u7 the Live line reads as words", () => {
  it("u7_the_live_text_has_a_space_between_the_label_and_the_age", async () => {
    await mountRail([
      agent("gateway", "error", "tests failed", { status: { kind: "error", label: "tests failed", since: NOW - 31 * MINUTE } }),
    ]);

    expect(rowOf("gateway").querySelector(".live")?.textContent).toBe("tests failed 31m");
  });

  it("u7_an_exited_terminals_live_text_has_no_trailing_space", async () => {
    await mountRail([terminal("dev")], [exitedTerminal("dev", 137)]);
    fireEvent.mouseEnter(rowOf("dev"));

    const text = rowOf("dev").querySelector(".live")?.textContent;

    expect(text).toBe(text?.trim());
  });
});
