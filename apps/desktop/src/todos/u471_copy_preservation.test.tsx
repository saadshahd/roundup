import { cleanup, fireEvent, screen, within } from "@solidjs/testing-library";
import { afterEach, expect, it } from "vitest";
import { workstream } from "../testing/nodes";
import { mountTodos, todo } from "./testHarness";

afterEach(cleanup);

it("u471_previous_copy_assertions_are_preserved_todo_creation_labels_keep_the_glossary_capital", async () => {
  await mountTodos([]);
  const add = screen.getByRole("button", { name: "Add Todo" });
  expect(add.textContent).toBe("Todo");
  fireEvent.click(add);
  expect(screen.getByRole("textbox", { name: "New Todo title" })).toBeTruthy();
});

it("u471_previous_copy_assertions_are_preserved_todo_drawer_controls_use_sentence_case", async () => {
  await mountTodos([todo(5, { title: "keep my CASE", body: "agent wrote THIS", blockers: [4], blocked: true }), todo(4)]);
  fireEvent.click(await screen.findByRole("button", { name: /#5 keep my CASE/ }));
  const drawer = within(screen.getByRole("complementary", { name: "Drawer" }));
  expect(await drawer.findByRole("checkbox", { name: "Complete keep my CASE" })).toBeTruthy();
  expect((await drawer.findByRole("button", { name: "Remove blocker #4" })).textContent).toBe("Remove");
  expect(drawer.getByText("Waits on")).toBeTruthy();
  expect(drawer.queryByText("Created by you")).toBeNull();
  expect(drawer.getByText("agent wrote THIS")).toBeTruthy();
  expect(drawer.getByRole("button", { name: "Blocker" })).toBeTruthy();
});

it("u471_previous_copy_assertions_are_preserved_todo_drawer_omits_move_and_home", async () => {
  await mountTodos([todo(5)], true, [workstream("g", { name: "workstream" })]);
  fireEvent.click(await screen.findByRole("button", { name: /#5/ }));
  const drawer = within(screen.getByRole("complementary", { name: "Drawer" }));
  await drawer.findByText("Blocker");
  expect(drawer.queryByRole("button", { name: "Move #5" })).toBeNull();
  expect(drawer.queryByText("In Project root")).toBeNull();
});

it.each(["title", "body"] as const)("u471_previous_copy_assertions_are_preserved_todo_empty_%s_and_editor_label_use_sentence_case", async (name) => {
  await mountTodos([todo(5, { [name]: "" })]);
  fireEvent.click(await screen.findByRole("button", { name: /#5/ }));
  const drawer = within(screen.getByRole("complementary", { name: "Drawer" }));
  fireEvent.dblClick(await drawer.findByText(`No ${name}`));
  expect(drawer.getByRole("textbox", { name: name === "title" ? "Title" : "Body" })).toBeTruthy();
});

it.each([
  { blocked: false, done: false, label: "Idle" },
  { blocked: true, done: false, label: "Blocked" },
  { blocked: false, done: true, label: "Done" },
])("u471_previous_copy_assertions_are_preserved_todo_row_status_uses_sentence_case_$label", async ({ label, ...status }) => {
  await mountTodos([todo(5, { ...status, title: "my TITLE" })]);

  if (status.done) fireEvent.click(await screen.findByRole("button", { name: "1 done" }));

  expect(await screen.findByRole("button", { name: `${label} #5 my TITLE` })).toBeTruthy();
});



it("u471_previous_copy_assertions_are_preserved_todo_empty_hint_keeps_the_glossary_capital", async () => {
  await mountTodos([]);

  expect(await screen.findByText("No Todos yet")).toBeTruthy();
});

it("u471_previous_copy_assertions_are_preserved_todo_delete_confirmation_uses_sentence_case", async () => {
  await mountTodos([todo(5)]);
  fireEvent.click(await screen.findByRole("button", { name: /#5/ }));
  const drawer = within(screen.getByRole("complementary", { name: "Drawer" }));

  fireEvent.click(await drawer.findByRole("button", { name: "Delete Todo" }));

  expect(await drawer.findByText("Delete Todo?", { exact: false })).toBeTruthy();
  expect(drawer.getByRole("button", { name: "Delete" })).toBeTruthy();
  expect(drawer.getByRole("button", { name: "Keep" })).toBeTruthy();
});
