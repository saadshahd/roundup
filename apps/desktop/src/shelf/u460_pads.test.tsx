import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, expect, it } from "vitest";
import { AGENT, openShelf, padOf } from "../pads/padsFixture";
import { USER } from "../testing/nodes";
import { askToDelete, confirmDeletion, stubDialogLayout } from "./bulkQuestionHarness";

afterEach(cleanup);

stubDialogLayout();

it("u460_agent_owned_pads_warn_and_delete_without_stopping_agents", async () => {
  const { app } = await openShelf([padOf("notes", AGENT, "a"), padOf("mine", USER, "b")]);
  fireEvent.click(await screen.findByRole("button", { name: "notes" }), { metaKey: true });
  fireEvent.click(screen.getByRole("button", { name: "mine" }), { metaKey: true });
  await askToDelete(2);
  expect(screen.getByText(/Agents may still be writing/).textContent).toContain("auth-refactor");
  expect(app.calls.filter((call) => call.method === "pad.delete")).toEqual([]);
});

it("u460_open_pad_drawer_observes_bulk_delete", async () => {
  const { app, connected } = await openShelf([padOf("notes", USER, "body")]);
  const row = await screen.findByRole("button", { name: "notes" });
  fireEvent.click(row);
  await screen.findByRole("button", { name: "Delete" });
  fireEvent.click(row, { metaKey: true });
  await askToDelete(1);
  await confirmDeletion();
  await screen.findByText(/Pad notes was deleted/);
  expect(connected.drawer.content()).not.toBeNull();
  expect(app.calls.filter((call) => call.method.startsWith("agent."))).toEqual([]);
});

it("u460_question_tracks_pad_owner_changes_without_expanding_targets", async () => {
  const { app, state } = await openShelf([padOf("notes", USER), padOf("mine", USER)]);
  fireEvent.click(await screen.findByRole("button", { name: "notes" }), { metaKey: true });
  await askToDelete(1);
  state.pads = [padOf("notes", AGENT), padOf("mine", USER)];
  app.emit({ actor: USER, name: "pad.changed", data: { name: "notes" } });
  await screen.findByText(/Agents may still be writing/);
  expect(screen.getByText("Delete 1 Pads?")).toBeTruthy();
  expect(app.calls.filter((call) => call.method === "pad.delete")).toEqual([]);
});
