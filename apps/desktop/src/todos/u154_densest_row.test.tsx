import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { room } from "../testing/nodes";
import rowCss from "./rowButton.styles.css?inline";
import appCss from "../styles.css?inline";
import { mountTodos, todo, todoRowOf as rowOf } from "./testHarness";

const sheet = document.createElement("style");

sheet.textContent = `${rowCss}\n${appCss}`;

beforeEach(() => document.head.appendChild(sheet));

afterEach(() => {
  cleanup();
  sheet.remove();
});

const LONG_ROOM = "r".repeat(60);

const densest = () => [
  todo(1, { title: "t".repeat(120), home: "r1" }),
  ...[2, 3, 4, 5, 6, 7, 8, 9, 10, 11].map((id) => todo(id)),
  todo(12, { blocked: true, blockers: [2, 3, 4, 5, 6, 7, 8, 9, 10, 11] }),
];

// jsdom has no layout, so the rules that let text wrap are read from the computed style; the harness observer measures `scrollWidth`.
const wraps = (el: Element) => {
  const style = getComputedStyle(el);

  expect(style.textOverflow).not.toBe("ellipsis");
  expect(style.overflow).not.toBe("hidden");
  expect(style.whiteSpace).not.toBe("nowrap");
};

describe("u154 the row on the densest data", () => {
  it("u154_the_title_the_waits_on_line_and_the_home_line_wrap_without_ellipsis_or_clipping", async () => {
    await mountTodos(densest(), true, [room("r1", { name: LONG_ROOM })]);
    await screen.findByText(/#12 /);

    const row = rowOf(12);

    expect(rowOf(1).querySelector("[data-todo-home]")?.textContent).toBe(`in ${LONG_ROOM}`);
    expect(row.querySelector("[data-todo-waits]")?.textContent).toBe("waits on #2, #3, #4, #5, #6, #7, #8, #9, #10, #11");

    for (const el of [rowOf(1), row, ...rowOf(1).querySelectorAll("*"), ...row.querySelectorAll("*")]) wraps(el);

    for (const line of [rowOf(1).querySelector("[data-todo-home]")!, row.querySelector("[data-todo-waits]")!])
      expect(getComputedStyle(line).overflowWrap).toBe("anywhere");

    expect(getComputedStyle(rowOf(1).querySelector(".todo-row-button")!).whiteSpace).toBe("pre-wrap");
  });

  it("u154_a_failure_takes_the_place_of_the_waits_on_line_and_the_home_line_stays", async () => {
    const { app } = await mountTodos(densest(), true, [room("r1", { name: LONG_ROOM })]);

    await screen.findByText(/#12 /);
    app.handlers["todo.complete"] = () => Promise.reject(new Error("could not complete the todo"));

    fireEvent.mouseEnter(rowOf(12));
    fireEvent.click(await screen.findByRole("button", { name: "complete" }));
    await screen.findByText("could not complete the todo");

    expect(rowOf(12).querySelector("[data-todo-waits]")).toBeNull();
    expect(rowOf(12).querySelector("[data-todo-home]")?.textContent).toBe("in project root");
  });
});
