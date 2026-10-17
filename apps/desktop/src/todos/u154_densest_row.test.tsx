import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { workstream } from "../testing/nodes";
import { mountTodos, todo, todoRowOf as rowOf } from "./testHarness";

afterEach(cleanup);

const LONG_ROOM = "r".repeat(60);

const densest = () => [
  todo(1, { title: "t".repeat(120), home: "r1" }),
  ...[2, 3, 4, 5, 6, 7, 8, 9, 10, 11].map((id) => todo(id)),
  todo(12, { blocked: true, blockers: [2, 3, 4, 5, 6, 7, 8, 9, 10, 11] }),
];

// U154: full names remain accessible; the rendered observer checks clipping and dense errors.
describe("u154 the row on the densest data", () => {
  it("u154_dense_titles_and_blockers_keep_their_accessible_names", async () => {
    await mountTodos(densest(), true, [workstream("r1", { name: LONG_ROOM })]);
    await screen.findByRole("button", { name: /#12 / });

    const row = rowOf(12);

    expect(rowOf(1).querySelector("[data-todo-home]")).toBeNull();
    expect(row.querySelector("[data-todo-waits]")?.textContent).toBe("Waits on todo 2, todo 3, todo 4, todo 5, todo 6, todo 7, todo 8, todo 9, todo 10, todo 11");
    expect(screen.getByRole("button", { name: `Idle #1 ${"t".repeat(120)}` })).toBeTruthy();
  });

  it("u154_a_failure_takes_the_place_of_the_waits_on_line_and_the_home_line_stays_absent", async () => {
    const { app } = await mountTodos(densest(), true, [workstream("r1", { name: LONG_ROOM })]);

    await screen.findByRole("button", { name: /#12 / });
    app.handlers["todo.complete"] = () => Promise.reject(new Error("could not complete the todo"));

    fireEvent.mouseEnter(rowOf(12));
    fireEvent.click(await screen.findByRole("checkbox", { name: "Complete todo 12" }));
    await screen.findByText("could not complete the todo");

    expect(rowOf(12).querySelector("[data-todo-waits]")).toBeNull();
    expect(rowOf(12).querySelector("[data-todo-home]")).toBeNull();
  });
});
