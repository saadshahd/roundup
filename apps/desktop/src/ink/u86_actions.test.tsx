import { cleanup, fireEvent, screen, within } from "@solidjs/testing-library";
import { afterEach, expect, it } from "vitest";
import { agent, room, USER } from "../testing/nodes";
import { mountRail, rowOf } from "../rail/railFixture";
import { openPad, openShelf, padOf, AGENT } from "../pads/padsFixture";
import { mountTodos, todo } from "../todos/testHarness";
import { resolveToken, tokensFrom, withStylesheets } from "../testing/contrast";
import tokens from "../tokens.css?inline";
import styles from "./styles.css?inline";

afterEach(cleanup);

const vector = (element: Element, name: string) => {
  const svg = element.querySelector(`svg.lucide-${name}`);
  expect(svg).not.toBeNull();
  expect(svg?.getAttribute("aria-hidden")).toBe("true");
};

it("u86_disclosure_uses_vectors_and_preserves_expansion", async () => {
  await mountRail([room("topic")]);
  const button = screen.getByRole("button", { name: "collapse" });
  vector(button, "chevron-down");
  fireEvent.click(button);
  vector(button, "chevron-right");
  expect(rowOf("topic").getAttribute("aria-expanded")).toBe("false");
  fireEvent.click(button);
  expect(rowOf("topic").getAttribute("aria-expanded")).toBe("true");
});

it("u86_rail_add_actions_keep_their_names", async () => {
  await mountRail([]);

  for (const name of ["agent", "terminal", "room"]) {
    vector(screen.getByRole("button", { name }), "plus");
  }
});

it("u86_todo_add_and_drawer_close_are_named_vector_actions", async () => {
  await mountTodos([todo(1)]);
  const add = screen.getByRole("button", { name: "add todo" });
  vector(add, "plus");
  fireEvent.click(add);
  expect(screen.getByRole("textbox", { name: "new todo title" })).toBeTruthy();
  fireEvent.click(await screen.findByRole("button", { name: /todo 1/ }));
  const drawer = screen.getByLabelText("drawer");
  vector(within(drawer).getByRole("button", { name: "close" }), "x");
  vector(await within(drawer).findByRole("button", { name: "blocker" }), "plus");
  expect(drawer.contains(document.activeElement)).toBe(true);
});

it("u86_pad_ownership_and_add_use_vectors_without_duplicate_drawer_names", async () => {
  await openShelf([padOf("notes", AGENT), padOf("draft", USER)]);
  vector(screen.getByRole("button", { name: "add pad" }), "plus");
  vector(screen.getByRole("button", { name: "make notes yours" }), "diamond-plus");
  vector(screen.getByRole("img", { name: "user owned" }), "diamond");
  await openPad("notes");
  const drawer = screen.getByLabelText("drawer");
  expect(within(drawer).queryByRole("img", { name: "agent owned" })).toBeNull();
  vector(drawer, "diamond-plus");
});

it("u86_rail_marks_share_the_disclosures_scan_line_when_selected", async () => {
  const sheet = document.head.appendChild(document.createElement("style"));
  sheet.textContent = styles;

  try {
    await withStylesheets(async () => {
      const { rail } = await mountRail([room("topic"), agent("a", "working", "w")]);
      const values = tokensFrom(tokens);

      for (const name of ["topic", "a"]) {
        const row = rowOf(name);

        for (const selected of [false, true]) {
          if (selected) rail.select(name);

          expect(row.getAttribute("aria-selected")).toBe(String(selected));
          const band = getComputedStyle(row);
          const line = getComputedStyle(row.querySelector(".line")!);
          const icon = getComputedStyle(row.querySelector("svg")!);
          const mark = getComputedStyle(row.querySelector(".line > button, .line > .glyph")!);

          expect(band.minHeight).toBe("28px");
          expect(resolveToken(line.minHeight === "inherit" ? band.minHeight : line.minHeight, values)).toBe("28px");
          expect(line.display).toBe("flex");
          expect(line.alignItems).toBe("center");
          expect([icon.width, icon.height].map((value) => resolveToken(value, values))).toEqual(["16px", "16px"]);
          expect(resolveToken(name === "topic" ? mark.minWidth : mark.width, values)).toBe("24px");
          expect(mark.justifyContent).toBe("center");

          if (name === "topic") expect(resolveToken(mark.minHeight, values)).toBe("24px");
        }
      }
    });
  } finally {
    sheet.remove();
  }
});
