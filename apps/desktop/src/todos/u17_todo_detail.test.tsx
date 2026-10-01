import { cleanup, fireEvent, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { RpcError } from "../app/seam";
import { callsTo, mountTodos, todo, todoEvent } from "./testHarness";

afterEach(cleanup);

const BLOCKED = todo(5, { title: "session store", body: "Move the cache.", blocked: true, blockers: [4] });

const SHELF = [todo(4, { title: "migrate users" }), BLOCKED, todo(10, { title: "session tests", blocked: true, blockers: [5] }), todo(7)];

const drawer = () => screen.getByRole("complementary", { name: "drawer" });

const openDrawerOf = async (title: RegExp, todos = SHELF) => {
  const mounted = await mountTodos(todos);
  fireEvent.click(await screen.findByRole("button", { name: title }));
  await within(drawer()).findByText("+ blocker");

  return mounted;
};

describe("u17 Todo detail", () => {
  it("u17_clicking_a_todo_opens_its_drawer_with_its_id_title_and_glyph", async () => {
    await openDrawerOf(/#5/);

    expect(drawer().textContent).toContain("⏸ #5  session store");
  });

  it("u17_waits_on_lists_each_blocker_with_its_own_glyph", async () => {
    await openDrawerOf(/#5/);

    expect(within(drawer()).getByText("waits on").nextElementSibling?.textContent).toBe("· #4  migrate users");
  });

  it("u17_a_done_blocker_still_shows_under_waits_on_with_the_done_glyph", async () => {
    await openDrawerOf(/#5/, [todo(4, { title: "migrate users", done: true }), BLOCKED]);

    expect(within(drawer()).getByText("waits on").nextElementSibling?.textContent).toBe("✓ #4  migrate users");
  });

  it("u17_blocks_lists_each_todo_that_waits_on_this_one", async () => {
    await openDrawerOf(/#5/);

    expect(within(drawer()).getByText("blocks").nextElementSibling?.textContent).toBe("⏸ #10  session tests");
  });

  it("u17_a_todo_with_no_blockers_has_no_waits_on_and_no_blocks_sections", async () => {
    await openDrawerOf(/#7/);

    expect([within(drawer()).queryByText("waits on"), within(drawer()).queryByText("blocks")]).toEqual([null, null]);
  });

  it("u17_the_drawer_shows_the_body", async () => {
    await openDrawerOf(/#5/);

    expect(within(drawer()).getByText("Move the cache.")).toBeTruthy();
  });

  it("u17_the_drawer_shows_the_last_touch_line", async () => {
    const { app } = await mountTodos(SHELF);
    app.handlers["provenance.history"] = () => [
      { actor: { kind: "user", id: "you", parent: null }, verb: "wrote", item: "todo:5", at: new Date(2026, 0, 5, 13, 10).getTime() },
    ];

    fireEvent.click(await screen.findByRole("button", { name: /#5/ }));

    expect((await within(drawer()).findByText(/^last/)).textContent).toBe("last  you wrote 13:10");
  });

  it("u17_opening_the_drawer_calls_history_then_todo_get_once", async () => {
    const { app } = await openDrawerOf(/#5/);

    expect(app.calls.map((call) => call.method).filter((method) => method.startsWith("provenance") || method === "todo.get")).toEqual([
      "provenance.history",
      "todo.get",
    ]);
    expect(callsTo(app, "provenance.history")[0]?.params).toEqual({ item: "todo:5" });
  });

  it("u17_a_background_refresh_never_calls_todo_get_again", async () => {
    const { app } = await openDrawerOf(/#5/);

    app.emit(todoEvent("todo.updated", BLOCKED));
    await waitFor(() => expect(callsTo(app, "todo.list")).toHaveLength(2));

    expect(callsTo(app, "todo.get")).toHaveLength(1);
  });

  it("u17_an_event_changes_what_the_open_drawer_shows", async () => {
    const { app, store } = await openDrawerOf(/#5/);
    store.todos = SHELF.map((each) => (each.id === 4 ? { ...each, done: true } : each));

    app.emit(todoEvent("todo.updated", todo(4)));

    await waitFor(() => expect(within(drawer()).getByText("waits on").nextElementSibling?.textContent).toBe("✓ #4  migrate users"));
  });

  it("u17_plus_blocker_offers_the_other_todos_that_are_not_yet_blockers", async () => {
    await openDrawerOf(/#5/);

    fireEvent.click(within(drawer()).getByText("+ blocker"));

    expect(within(drawer()).getAllByRole("button").map((button) => button.textContent).filter((text) => text?.includes("#"))).toEqual([
      "· #7  todo 7",
      "⏸ #10  session tests",
    ]);
  });

  it("u17_choosing_an_offered_todo_calls_set_blockers_with_the_whole_new_list", async () => {
    const { app } = await openDrawerOf(/#5/);
    app.handlers["todo.setBlockers"] = () => BLOCKED;
    fireEvent.click(within(drawer()).getByText("+ blocker"));

    fireEvent.click(within(drawer()).getByRole("button", { name: /#7/ }));

    await waitFor(() =>
      expect(callsTo(app, "todo.setBlockers")).toEqual([{ method: "todo.setBlockers", params: { id: 5, blockers: [4, 7] } }]),
    );
  });

  it("u17_a_conflict_from_set_blockers_shows_inline", async () => {
    const { app } = await openDrawerOf(/#5/);
    app.handlers["todo.setBlockers"] = () => Promise.reject(new RpcError(-32010, "#10 already waits on #5"));
    fireEvent.click(within(drawer()).getByText("+ blocker"));

    fireEvent.click(within(drawer()).getByRole("button", { name: /#10/ }));

    expect((await within(drawer()).findByText(/already waits/)).textContent).toBe("✕ #10 already waits on #5");
  });

  it("u17_complete_calls_todo_complete", async () => {
    const { app } = await openDrawerOf(/#5/);
    app.handlers["todo.complete"] = () => BLOCKED;

    fireEvent.click(within(drawer()).getByText("complete"));

    await waitFor(() => expect(callsTo(app, "todo.complete")).toEqual([{ method: "todo.complete", params: { id: 5 } }]));
  });

  it("u17_delete_calls_todo_delete_and_closes_the_drawer", async () => {
    const { app, workspace } = await openDrawerOf(/#5/);
    app.handlers["todo.delete"] = () => null;

    fireEvent.click(within(drawer()).getByText("delete"));

    await waitFor(() => expect(workspace.drawer.content()).toBeNull());
    expect(callsTo(app, "todo.delete")).toEqual([{ method: "todo.delete", params: { id: 5 } }]);
  });

  it("u17_a_failed_delete_keeps_the_drawer_open_and_shows_the_message", async () => {
    const { app, workspace } = await openDrawerOf(/#5/);
    app.handlers["todo.delete"] = () => Promise.reject(new RpcError(-32001, "not found"));

    fireEvent.click(within(drawer()).getByText("delete"));

    expect([(await within(drawer()).findByText(/not found/)).textContent, workspace.drawer.content() === null]).toEqual([
      "✕ not found",
      false,
    ]);
  });

  it("u17_double_clicking_the_title_edits_it_and_leaving_calls_todo_update_once", async () => {
    const { app } = await openDrawerOf(/#5/);
    app.handlers["todo.update"] = () => BLOCKED;
    fireEvent.dblClick(within(drawer()).getByText("session store"));
    const field = within(drawer()).getByRole("textbox", { name: "title" });

    fireEvent.input(field, { target: { value: "session cache" } });
    fireEvent.blur(field);
    fireEvent.blur(field);

    expect(callsTo(app, "todo.update")).toEqual([{ method: "todo.update", params: { id: 5, title: "session cache", body: null } }]);
  });

  it("u17_enter_in_the_title_field_leaves_it", async () => {
    const { app } = await openDrawerOf(/#5/);
    app.handlers["todo.update"] = () => BLOCKED;
    fireEvent.dblClick(within(drawer()).getByText("session store"));
    const field = within(drawer()).getByRole("textbox", { name: "title" });
    field.focus();

    fireEvent.input(field, { target: { value: "session cache" } });
    fireEvent.keyDown(field, { key: "Enter" });

    await waitFor(() => expect(callsTo(app, "todo.update")).toHaveLength(1));
  });

  it("u17_double_clicking_the_body_edits_it_and_leaving_calls_todo_update_with_the_body", async () => {
    const { app } = await openDrawerOf(/#5/);
    app.handlers["todo.update"] = () => BLOCKED;
    fireEvent.dblClick(within(drawer()).getByText("Move the cache."));
    const field = within(drawer()).getByRole("textbox", { name: "body" });

    fireEvent.input(field, { target: { value: "Move it to redis." } });
    fireEvent.blur(field);

    expect(callsTo(app, "todo.update")).toEqual([{ method: "todo.update", params: { id: 5, title: null, body: "Move it to redis." } }]);
  });

  it("u17_leaving_a_field_with_unchanged_text_calls_nothing", async () => {
    const { app } = await openDrawerOf(/#5/);
    fireEvent.dblClick(within(drawer()).getByText("session store"));

    fireEvent.blur(within(drawer()).getByRole("textbox", { name: "title" }));

    expect(callsTo(app, "todo.update")).toEqual([]);
  });
});
