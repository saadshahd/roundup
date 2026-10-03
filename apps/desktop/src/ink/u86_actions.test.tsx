import { cleanup, fireEvent, screen, within } from "@solidjs/testing-library";
import { afterEach, expect, it } from "vitest";
import { group, USER } from "../testing/nodes";
import { mountRail, rowOf } from "../rail/railFixture";
import { openPad, openShelf, padOf, AGENT } from "../pads/padsFixture";
import { mountTodos, todo } from "../todos/testHarness";

afterEach(cleanup);

const vector = (element: Element, name: string) => {
  const svg = element.querySelector(`svg.lucide-${name}`);
  expect(svg).not.toBeNull();
  expect(svg?.getAttribute("aria-hidden")).toBe("true");
};

it("u86_disclosure_uses_vectors_and_preserves_expansion", async () => {
  await mountRail([group("topic")]);
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

  for (const name of ["agent", "terminal", "group"]) {
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
