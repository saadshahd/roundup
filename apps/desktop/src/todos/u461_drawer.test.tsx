import { stubDialogLayout } from "../shelf/bulkQuestionHarness";
import { cleanup, fireEvent, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, expect, it } from "vitest";
import { RpcError } from "../app/seam";
import { mountTodos, todo, todoCallsTo, todoEvent } from "./testHarness";

afterEach(cleanup);

stubDialogLayout();

const panel = () => screen.getByRole("complementary", { name: "Drawer" });

const open = async () => {
  const mounted = await mountTodos([todo(1, { title: "Ship", body: "Read this\nThen this" })]);
  fireEvent.click(await screen.findByRole("button", { name: "Idle #1 Ship" }));
  await within(panel()).findByText("Blocker");

  return mounted;
};

it("u461_header_and_content_have_the_requested_hierarchy", async () => {
  await open();
  const box = within(panel()).getByRole<HTMLInputElement>("checkbox", { name: "Complete Ship" });
  const title = within(panel()).getByText("Ship");
  expect(box.compareDocumentPosition(title) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(box.classList.contains("todo-tick")).toBe(true);
  expect(within(panel()).getByRole("button", { name: "Delete Todo" }).querySelector("svg")).toBeTruthy();

  for (const text of ["#1", "Created by", "Last", "In Project root", "Move", "Complete"]) expect(panel().textContent).not.toContain(text);
});

it("u461_completion_failure_keeps_focus", async () => {
  const { app } = await open();
  app.handlers["todo.complete"] = () => Promise.reject(new RpcError(-32003, "Try again"));
  const box = within(panel()).getByRole<HTMLInputElement>("checkbox", { name: "Complete Ship" });
  box.focus(); fireEvent.click(box);
  await within(panel()).findByText("Try again");
  expect(box.checked).toBe(false); expect(document.activeElement).toBe(box);
});

it("u461_completion_success_keeps_drawer_focus", async () => {
  const { app, store } = await open();
  const box = within(panel()).getByRole<HTMLInputElement>("checkbox", { name: "Complete Ship" });
  box.focus(); fireEvent.click(box);
  await waitFor(() => expect(todoCallsTo(app, "todo.complete")).toHaveLength(1));
  app.emit(todoEvent("todo.updated", store.todos[0]!));
  await waitFor(() => expect((box).checked).toBe(true));
  expect(panel().contains(document.activeElement)).toBe(true);
});

it("u461_keep_and_escape_restore_focus", async () => {
  const { app } = await open();
  const button = within(panel()).getByRole("button", { name: "Delete Todo" });
  fireEvent.click(button);
  await waitFor(() => expect(document.activeElement).toBe(within(panel()).getByRole("button", { name: "Keep" })));
  fireEvent.keyDown(document.activeElement!, { key: "Escape", bubbles: true });
  await waitFor(() => expect(document.activeElement).toBe(button));
  await waitFor(() => expect(within(panel()).queryByRole("button", { name: "Keep" })).toBeNull());
  expect(todoCallsTo(app, "todo.delete")).toHaveLength(0);
});

it("u461_done_is_checked", async () => {
  const { app } = await mountTodos([todo(1, { title: "Done Todo", done: true })]);

  fireEvent.click(await screen.findByRole("button", { name: "1 done" }));
  fireEvent.click(await screen.findByRole("button", { name: "Done #1 Done Todo" }));
  const box = await within(panel()).findByRole<HTMLInputElement>("checkbox", { name: "Complete Done Todo" });

  expect(box.checked).toBe(true);
  expect(box.disabled).toBe(true);
  fireEvent.click(box);

  expect(todoCallsTo(app, "todo.complete")).toHaveLength(0);
});

it("u461_delete_failure_is_retryable", async () => {
  const { app, connected } = await open();
  let reject!: (error: RpcError) => void;
  app.handlers["todo.delete"] = () => new Promise((_, fail) => { reject = fail; });

  fireEvent.click(within(panel()).getByRole("button", { name: "Delete Todo" }));
  const confirm = await within(panel()).findByRole("button", { name: "Delete" });
  fireEvent.click(confirm);
  fireEvent.click(confirm);

  expect(todoCallsTo(app, "todo.delete")).toHaveLength(1);
  reject(new RpcError(-32003, "Delete failed"));
  await within(panel()).findByText("Delete failed");
  app.handlers["todo.delete"] = () => null;
  fireEvent.click(confirm);

  await waitFor(() => expect(connected.drawer.content()).toBeNull());
  expect(todoCallsTo(app, "todo.delete")).toHaveLength(2);
});

it("u461_pad_drawer_keeps_its_touch_line", async () => {
  const { openShelf, openPad, padOf } = await import("../pads/padsFixture");
  const { USER } = await import("../testing/nodes");
  const { state } = await openShelf([padOf("Notes", USER, "Read me")]);
  state.history = [{ actor: USER, verb: "wrote", item: "pad:Notes", at: new Date(2026, 0, 5, 14, 22).getTime() }];

  await openPad("Notes");

  expect(await screen.findByText(/You wrote this Pad at/)).toBeTruthy();
});
