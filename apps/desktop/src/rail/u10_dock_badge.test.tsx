import { cleanup } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { mountRail } from "./railFixture";
import { agent, event, workstream, door, terminal } from "../testing/nodes";

afterEach(cleanup);

describe("u10 Dock badge", () => {
  it("u10_the_badge_counts_agents_needing_you_or_in_error", async () => {
    const { app } = await mountRail([
      agent("a", "needs-you", "x"),
      agent("b", "error", "x"),
      agent("c", "blocked", "x"),
      agent("d", "working", "x"),
    ]);

    expect(app.badges.at(-1)).toBe(2);
  });

  it("u10_there_is_no_badge_at_zero", async () => {
    const { app } = await mountRail([agent("a", "working", "x"), terminal("t"), workstream("g")]);

    expect(app.badges).toEqual([0]);
  });

  it("u10_a_meta_agent_that_needs_you_counts", async () => {
    const { app } = await mountRail([door("lead", "needs-you", "x")]);

    expect(app.badges.at(-1)).toBe(1);
  });

  it("u10_the_badge_follows_agent_status_events", async () => {
    const { app, rail } = await mountRail([agent("a", "working", "x"), agent("b", "working", "x")]);

    app.emit(event({ name: "agent.status", data: { status_revision: "2", attempt: "1", id: "a", status: { kind: "needs-you", label: "?", since: 1 } } }));
    app.emit(event({ name: "agent.status", data: { status_revision: "2", attempt: "1", id: "b", status: { kind: "error", label: "!", since: 1 } } }));
    await rail.settled();
    app.emit(event({ name: "agent.status", data: { status_revision: "3", attempt: "1", id: "a", status: { kind: "working", label: "w", since: 2 } } }));
    await rail.settled();

    expect(app.badges).toEqual([0, 1, 2, 1]);
  });

  it("u10_the_badge_is_not_resent_when_the_count_is_unchanged", async () => {
    const { app, rail } = await mountRail([agent("a", "needs-you", "x")]);

    app.emit(event({ name: "agent.status", data: { status_revision: "2", attempt: "1", id: "a", status: { kind: "error", label: "!", since: 1 } } }));
    await rail.settled();

    expect(app.badges).toEqual([1]);
  });
});
