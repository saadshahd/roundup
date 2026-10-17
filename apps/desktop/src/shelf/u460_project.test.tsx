import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, expect, it } from "vitest";
import { fakeEmulators } from "../terminal/paneHarness";
import { App } from "../App";
import { seedApp } from "../testing/seeds";
import { showEverythingInApp } from "../testing/showEverything";
import { deferred } from "../pads/padsFixture";
import { RpcError } from "../app/seam";
import { askToDelete, confirmDeletion, stubDialogLayout } from "./bulkQuestionHarness";

afterEach(() => { cleanup(); localStorage.clear(); });

stubDialogLayout();

it("u460_pending_delete_stays_with_its_project", async () => {
  const { app } = seedApp("tree-40", 0);
  app.of("/Users/you/harness");
  app.add("/other", "other");
  const other = app.of("/other");
  render(() => <App app={app} reducedMotion={() => true} clock={() => 0} createEmulator={fakeEmulators().factory} />);
  fireEvent.click(await screen.findByRole("tab", { name: "harness" }));
  await showEverythingInApp();
  const list = screen.getByRole("region", { name: "Todos" });
  const rows = within(list).getAllByRole("button").filter((row) => row.classList.contains("todo-row-button") && row.hasAttribute("aria-pressed"));
  fireEvent.click(rows[0]!, { metaKey: true });
  fireEvent.click(rows[1]!, { metaKey: true });
  const held = deferred<null>();
  app.handlers["todo.delete"] = async () => { await held.promise; throw new RpcError(-32001, "old Project error"); };

  await askToDelete(2);

  await confirmDeletion();
  fireEvent.click(screen.getByRole("tab", { name: "other" }));
  held.resolve(null);
  await waitFor(() => expect(app.calls.filter((call) => call.method === "todo.delete")).toHaveLength(2));
  expect(other.calls.filter((call) => call.method === "todo.delete")).toEqual([]);
  expect(screen.queryByText(/old Project error/)).toBeNull();
  expect(screen.queryByText("2 selected")).toBeNull();
}, 20_000);

it("u460_escape_from_a_shelf_tab_clears_selection_before_closing_the_shelf", async () => {
  const { app } = seedApp("tree-40", 0);
  render(() => <App app={app} reducedMotion={() => true} clock={() => 0} createEmulator={fakeEmulators().factory} />);
  await showEverythingInApp();
  const row = document.querySelector<HTMLButtonElement>(".todo-row-button[aria-pressed]")!;
  fireEvent.click(row, { metaKey: true });
  const tab = within(screen.getByRole("tablist", { name: "Work items" })).getByRole("tab", { name: /Todos/ });
  tab.focus();
  fireEvent.keyDown(tab, { key: "Escape" });
  expect(screen.queryByText("1 selected")).toBeNull();
  expect(document.querySelector<HTMLElement>(".shelf")!.hidden).toBe(false);
  expect(document.activeElement).toBe(row);
}, 20_000);
