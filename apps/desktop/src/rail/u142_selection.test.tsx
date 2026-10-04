import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, expect, it, vi } from "vitest";
import { agent, group, MINUTE, NOW, terminal } from "../testing/nodes";
import { mountRail, railCallsTo, rowOf } from "./railFixture";
import { stubLayout } from "./dragFixture";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

it("u142_rename_pointer_selection_does_not_move_and_enter_commits_once", async () => {
  stubLayout();
  const { app } = await mountRail([group("backend")]);
  app.handlers["rail.rename"] = () => group("backend", { name: "new home" });
  fireEvent.dblClick(screen.getByText("backend"));
  const field = screen.getByRole<HTMLInputElement>("textbox", { name: "name" });
  await waitFor(() => expect(document.activeElement).toBe(field));
  expect([field.selectionStart, field.selectionEnd]).toEqual([0, 7]);
  fireEvent.pointerDown(field, { button: 0, clientX: 10, clientY: 10 });
  fireEvent.pointerMove(window, { clientX: 100, clientY: 10 });
  fireEvent.pointerUp(window, { clientX: 100, clientY: 10 });
  fireEvent.input(field, { target: { value: "new home" } });
  fireEvent.keyDown(field, { key: "Enter" });
  expect(railCallsTo(app, "rail.rename")).toEqual([{ id: "backend", name: "new home" }]);
  expect(railCallsTo(app, "rail.move")).toEqual([]);
});

it("u142_escape_keeps_the_name_without_rename_or_move", async () => {
  const { app } = await mountRail([group("backend")]);
  fireEvent.dblClick(screen.getByText("backend"));
  const field = screen.getByRole("textbox", { name: "name" });
  fireEvent.input(field, { target: { value: "other home" } });
  fireEvent.keyDown(field, { key: "Escape" });
  expect(screen.getByText("backend")).toBeTruthy();
  expect(railCallsTo(app, "rail.rename")).toEqual([]);
  expect(railCallsTo(app, "rail.move")).toEqual([]);
});

it.each([
  ["agent", "agent.spawn", { cwd: "/p", prompt: null, parent: "backend" }],
  ["terminal", "rail.spawnTerminal", { cwd: "/p", parent: "backend" }],
  ["group", "rail.createGroup", { name: "group", parent: "backend" }],
] as const)("u142_%s_control_keeps_its_accessible_name_and_action", async (name, method, params) => {
  const { app } = await mountRail([group("backend")]);
  app.handlers[method] = () => group("fresh");
  fireEvent.click(screen.getByText("backend"));
  const button = screen.getByRole("button", { name });
  button.focus();
  expect(document.activeElement).toBe(button);
  fireEvent.click(button);
  await waitFor(() => expect(railCallsTo(app, method)).toEqual([params]));
});

it("u142_rows_and_folds_keep_roles_clicks_and_keyboard_focus", async () => {
  const { rail } = await mountRail([
    group("backend"),
    terminal("shell", { parent: "backend" }),
    agent("finished", "done", "finished", {
      parent: "backend", order: 1,
      status: { kind: "done", label: "finished", since: NOW - 60 * MINUTE },
    }),
  ]);

  expect(screen.getByRole("tree", { name: "rail" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "1 done" }));
  expect(rowOf("finished").getAttribute("role")).toBe("treeitem");
  fireEvent.click(screen.getByText("backend"));
  expect(rail.selected()).toBe("backend");
  rowOf("backend").focus();
  fireEvent.keyDown(rowOf("backend"), { key: "ArrowDown" });
  expect(document.activeElement).toBe(rowOf("shell"));
  fireEvent.keyDown(rowOf("shell"), { key: "Enter" });
  expect(rail.selected()).toBe("shell");
  fireEvent.click(screen.getByRole("button", { name: "collapse" }));
  expect(screen.queryByText("shell")).toBeNull();
  expect(rowOf("backend").getAttribute("aria-expanded")).toBe("false");
});
