import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import styles from "../styles.css?inline";
import { mountTodos, todo, todoRowOf as rowOf } from "./testHarness";

afterEach(cleanup);

const longest = "a title that is as long as the row can hold before it has to wrap";

const shelf = () => [todo(1, { title: longest }), todo(2, { title: "short" }), todo(3, { title: longest, blocked: true, blockers: [2] })];

// jsdom has no layout: what it can read is everything layout depends on, i.e. each row's flow children, their text and their computed position.
// The browser proof (Chromium, `tree-40`, 500-1400 px) reads the top edge and height themselves.
const flowOf = (id: number) => {
  const row = rowOf(id);

  return [...row.querySelectorAll<HTMLElement>("*")]
    .filter((element) => !["absolute", "fixed"].includes(getComputedStyle(element).position))
    .map((element) => `${element.tagName}.${element.getAttribute("class")}:${[...element.childNodes].filter((node) => node.nodeType === Node.TEXT_NODE).map((node) => node.textContent).join("")}`);
};

const restOf = () => [1, 2, 3].map((id) => ({ id, flow: flowOf(id), title: rowOf(id).querySelector("button")!.textContent }));

const withStyles = async (run: () => Promise<void>) => {
  const sheet = document.head.appendChild(document.createElement("style"));
  sheet.textContent = styles;

  try {
    await mountTodos(shelf());
    await screen.findByRole("button", { name: /#1/ });
    await run();
  } finally {
    sheet.remove();
  }
};

describe("u112 hovering or focusing a Shelf row never moves a row", () => {
  it("u112_hovering_a_row_adds_nothing_to_any_rows_flow_or_title", async () => {
    await withStyles(async () => {
      const rest = restOf();

      for (const id of [1, 2, 3]) {
        fireEvent.mouseEnter(rowOf(id));
        expect(restOf()).toEqual(rest);
        expect(rowOf(id).querySelector(".complete")).not.toBeNull();
        fireEvent.mouseLeave(rowOf(id));
      }
    });
  });

  it("u112_focusing_a_row_or_its_complete_adds_nothing_to_any_rows_flow_or_title", async () => {
    await withStyles(async () => {
      const rest = restOf();

      for (const id of [1, 2, 3]) {
        const row = rowOf(id).querySelector<HTMLElement>(".todo-row-button")!;

        row.focus();
        expect(restOf()).toEqual(rest);
        rowOf(id).querySelector<HTMLElement>(".complete")!.focus();
        expect(restOf()).toEqual(rest);
        (document.activeElement as HTMLElement).blur();
      }
    });
  });

  it("u112_every_word_a_row_shows_on_hover_or_focus_is_out_of_flow_at_the_rows_right_edge", async () => {
    await withStyles(async () => {
      const before = rowOf(1).querySelectorAll("*").length;

      fireEvent.mouseEnter(rowOf(1));
      const added = [...rowOf(1).querySelectorAll<HTMLElement>("*")].filter((element) => element.classList.contains("complete"));
      const shown = getComputedStyle(added[0]!);

      expect([rowOf(1).querySelectorAll("*").length - before, added.length, shown.position, shown.top, shown.right]).toEqual([
        1,
        1,
        "absolute",
        "0px",
        "0px",
      ]);
    });
  });
});
