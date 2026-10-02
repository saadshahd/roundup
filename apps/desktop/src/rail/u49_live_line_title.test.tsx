import { cleanup } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { rowOf, mountRail } from "./railFixture";
import { agent, MINUTE, NOW } from "../testing/nodes";

afterEach(cleanup);

const asking = (over = {}) =>
  agent("gateway", "needs-you", "asks: keep v1 routes? please decide before the release", {
    status: { kind: "needs-you", label: "asks: keep v1 routes? please decide before the release", since: NOW - 4 * MINUTE },
    ...over,
  });

describe("u49 a live line never hides what the agent asked", () => {
  it("u49_the_live_lines_title_is_the_full_label_and_elapsed_time", async () => {
    await mountRail([asking()]);

    const live = rowOf("gateway").querySelector(".live");

    expect(live?.getAttribute("title")).toBe("asks: keep v1 routes? please decide before the release  4m");
  });
});
