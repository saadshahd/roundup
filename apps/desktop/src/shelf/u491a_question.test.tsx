import { cleanup, fireEvent, screen, within, waitFor } from "@solidjs/testing-library";
import { afterEach, expect, it } from "vitest";
import { stubDialogLayout } from "./bulkQuestionHarness";
import { mountTodos, todo, todoCallsTo } from "../todos/testHarness";

afterEach(cleanup);

stubDialogLayout();

it("u491a_bulk_question_focus_names_the_frozen_targets_and_cancels", async () => {
  const { app } = await mountTodos([todo(1), todo(2), todo(3)]);
  fireEvent.click(await screen.findByRole("button", { name: /#1 / }), { metaKey: true });
  fireEvent.click(screen.getByRole("button", { name: /#2 / }), { metaKey: true });
  const opener = screen.getByRole("button", { name: "Delete 2" });
  opener.focus();
  fireEvent.click(opener);
  const question = await screen.findByRole("alertdialog", { name: "Delete 2 Todos?" });
  expect(within(question).getByText("todo 1 #1")).toBeTruthy();
  expect(within(question).getByText("todo 2 #2")).toBeTruthy();
  await waitFor(() => expect(document.activeElement).toBe(within(question).getByRole("button", { name: "Keep" })));
  fireEvent.click(within(question).getByRole("button", { name: "Keep" }));
  await waitFor(() => expect(document.activeElement).toBe(opener));
  expect(todoCallsTo(app, "todo.delete")).toEqual([]);
});
