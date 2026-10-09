import { describe, expect, it } from "vitest";
import type { RailNode } from "@contracts/agent/RailNode";
import { connectEvents } from "../app/events";
import { createFakeApp } from "../testing/fakeApp";
import { createRailState } from "../state/rail";
import type { ExitState } from "../state/rail";
import { exitText } from "./exitText";

describe("u7 exited wording", () => {
  it.each([
    [{ kind: "code", code: 3 }, "exited 3"],
    [{ kind: "code", code: 0 }, "exited 0"],
    [{ kind: "signal" }, "exited by signal"],
    [{ kind: "unknown" }, "exited"],
  ] as const satisfies readonly (readonly [ExitState, string])[])("u7_%j_reads_%s", (exit, text) => {
    expect(exitText(exit)).toBe(text);
  });

  it("u7_the_three_exits_of_the_rail_state_read_three_different_ways", async () => {
    const app = createFakeApp();

    const nodes: RailNode[] = ["code", "signal", "none"].map((id, order) => ({
      id,
      kind: "terminal",
      name: id,
      parent: null,
      order,
      status: null,
      attempt: "1", status_revision: null,
      terminal_id: id === "none" ? null : `t-${id}`,
      worktree: null,
      can_resume: false,
      channel: null,
    }));

    app.handlers["rail.tree"] = () => nodes;
    const rail = createRailState(app, await connectEvents(app));
    await rail.settled();
    const actor = { kind: "user", id: "you", parent: null } as const;

    app.emit({ actor, name: "terminal.exited", data: { id: "t-code", code: 2 } });
    app.emit({ actor, name: "terminal.exited", data: { id: "t-signal", code: null } });

    expect(
      nodes.map((node) => {
        const exit = rail.exitOf(node);

        return exit === null ? null : exitText(exit);
      }),
    ).toEqual(["exited 2", "exited by signal", "exited"]);
  });
});
