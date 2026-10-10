import { cleanup, fireEvent, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import type { RailNode } from "@contracts/agent/RailNode";
import { door, event, workstream } from "../testing/nodes";
import { mountRail, pinnedAdd, railCallsTo, rowNames } from "./railFixture";

afterEach(cleanup);

const createdNames = (app: Parameters<typeof railCallsTo>[0]): unknown[] => railCallsTo(app, "rail.createWorkstream");

const named = (...names: string[]) => names.map((name) => ({ name, parent: null }));

describe("u167 a new Workstream's default name is unique among the Rail's roots", () => {
  it("u167_three_creations_then_a_rename_then_a_fourth_send_the_first_free_names", async () => {
    const tree: RailNode[] = [];
    const mounted = await mountRail(tree);
    let n = 0;
    mounted.app.handlers["rail.createWorkstream"] = () => door(`w${++n}`, "idle", "idle");
    mounted.app.handlers["rail.startDoor"] = () => door("x", "working", "starting");

    const create = async (count: number, name: string) => {
      fireEvent.click(pinnedAdd());
      await waitFor(() => expect(railCallsTo(mounted.app, "rail.createWorkstream")).toHaveLength(count));
      tree.push(workstream(`w${count}`, { name }));
      mounted.app.emit(event({ name: "rail.changed" }));
      await waitFor(() => expect(mounted.rail.selected()).toBe(`w${count}`));
      await mounted.rail.settled();
    };

    await create(1, "workstream");
    await create(2, "workstream 2");
    await create(3, "workstream 3");

    expect(createdNames(mounted.app)).toEqual(named("workstream", "workstream 2", "workstream 3"));
    expect(rowNames()).toEqual(expect.arrayContaining(["workstream", "workstream 2", "workstream 3"]));

    tree[1] = workstream("w2", { name: "renamed" });
    mounted.app.emit(event({ name: "rail.changed" }));
    await waitFor(() => expect(rowNames()).toContain("renamed"));
    await mounted.rail.settled();

    fireEvent.click(pinnedAdd());
    await waitFor(() => expect(createdNames(mounted.app)).toEqual(named("workstream", "workstream 2", "workstream 3", "workstream 2")));
  });

  it("u167_the_new_name_is_the_one_thread_and_card_words_use", async () => {
    const mounted = await mountRail([workstream("a", { name: "workstream" }), workstream("b", { name: "workstream 2" })]);

    expect(mounted.rail.nameOf({ kind: "agent", id: "b", parent: null })).toBe("workstream 2");
    expect(mounted.rail.nameOf({ kind: "agent", id: "a", parent: null })).toBe("workstream");
  });
});
