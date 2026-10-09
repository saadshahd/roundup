import { cleanup, fireEvent, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RpcError } from "../app/seam";
import { assertKind, mountTodos, todo, todoCallsTo as callsTo, todoEvent } from "./testHarness";
import { USER } from "../testing/nodes";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

const blocked = () => todo(5, { title: "session store", body: "Move the cache.", blocked: true, blockers: [4] });

/** Fresh objects per test: the list state reconciles into the objects it is given. */
const shelf = () => [todo(4, { title: "migrate users" }), blocked(), todo(10, { title: "session tests", blocked: true, blockers: [5] }), todo(7)];

const drawer = () => screen.getByRole("complementary", { name: "drawer" });

// A regex like /#4/ matches both a blocked Todo's own row ("#4 migrate users") and its
// blocker's "#4" link; the row is the candidate that is not that light "word" link.
const rowButton = async (title: RegExp): Promise<HTMLElement> => {
  const row = (await screen.findAllByRole("button", { name: title })).find((candidate) => candidate.className !== "word");

  if (!row) throw new Error(`no row for ${title}`);

  return row;
};

const openDrawerOf = async (title: RegExp, todos = shelf(), reducedMotion = true) => {
  const mounted = await mountTodos(todos, reducedMotion);
  fireEvent.click(await rowButton(title));
  await within(drawer()).findByText("blocker");

  return mounted;
};

describe("u17 Todo detail", () => {
  it("u17_clicking_a_todo_opens_its_drawer_with_its_id_title_and_glyph", async () => {
    await openDrawerOf(/#5/);

    expect(drawer().textContent).toContain("#5  session store");
    assertKind(within(drawer()).getByText("session store").parentElement!, "blocked", "pause");
  });

  it("u17_the_title_row_keeps_a_space_between_the_glyph_and_the_id", async () => {
    await openDrawerOf(/#5/);

    expect(within(drawer()).getByText((_, element) => element?.tagName === "SPAN" && element.textContent === " #5  ")).toBeTruthy();
  });

  it("u17_waits_on_lists_each_blocker_with_its_own_glyph", async () => {
    await openDrawerOf(/#5/);

    expect(within(drawer()).getByText("waits on").nextElementSibling?.textContent).toBe(" #4  migrate users");
    assertKind(within(drawer()).getByText("#4 migrate users"), "idle", "dot");
  });

  it("u17_a_done_blocker_still_shows_under_waits_on_with_the_done_glyph", async () => {
    await openDrawerOf(/#5/, [todo(4, { title: "migrate users", done: true }), blocked()]);

    expect(within(drawer()).getByText("waits on").nextElementSibling?.textContent).toBe(" #4  migrate users");
    assertKind(within(drawer()).getByText("#4 migrate users"), "done", "check");
  });

  it("u17_blocks_lists_each_todo_that_waits_on_this_one", async () => {
    await openDrawerOf(/#5/);

    expect(within(drawer()).getByText("blocks").nextElementSibling?.textContent).toBe(" #10  session tests");
    assertKind(within(drawer()).getByText("#10 session tests"), "blocked", "pause");
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
    const { app } = await mountTodos(shelf());
    app.handlers["provenance.history"] = () => [
      { actor: { kind: "user", id: "you", parent: null }, verb: "wrote", item: "todo:5", at: new Date(2026, 0, 5, 13, 10).getTime() },
    ];

    fireEvent.click(await rowButton(/#5/));

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
    const { app, store } = await openDrawerOf(/#5/);
    store.todos = shelf().map((each) => (each.id === 5 ? { ...each, title: "renamed" } : each));

    app.emit(todoEvent("todo.updated", blocked()));
    await within(drawer()).findByText("renamed");

    expect(callsTo(app, "todo.get")).toHaveLength(1);
  });

  it("u17_an_event_changes_what_the_open_drawer_shows", async () => {
    const { app, store } = await openDrawerOf(/#5/);
    store.todos = shelf().map((each) => (each.id === 4 ? { ...each, done: true } : each));

    app.emit(todoEvent("todo.updated", todo(4)));

    await waitFor(() => assertKind(within(drawer()).getByText("#4 migrate users"), "done", "check"));
  });

  it("u17_plus_blocker_offers_the_other_todos_that_are_not_yet_blockers", async () => {
    await openDrawerOf(/#5/);

    fireEvent.click(within(drawer()).getByText("blocker"));

    expect(within(drawer()).getAllByRole("button").map((button) => button.textContent).filter((text) => text?.includes("#"))).toEqual([
      " #7  todo 7",
      " #10  session tests",
    ]);
  });

  it("u17_choosing_an_offered_todo_calls_set_blockers_with_the_whole_new_list", async () => {
    const { app } = await openDrawerOf(/#5/);
    app.handlers["todo.setBlockers"] = () => blocked();
    fireEvent.click(within(drawer()).getByText("blocker"));

    fireEvent.click(within(drawer()).getByRole("button", { name: /#7/ }));

    await waitFor(() =>
      expect(callsTo(app, "todo.setBlockers")).toEqual([{ method: "todo.setBlockers", params: { id: 5, blockers: [4, 7] } }]),
    );
  });

  it("u17_a_conflict_from_set_blockers_shows_inline", async () => {
    const { app } = await openDrawerOf(/#5/);
    app.handlers["todo.setBlockers"] = () => Promise.reject(new RpcError(-32010, "#10 already waits on #5"));
    fireEvent.click(within(drawer()).getByText("blocker"));

    fireEvent.click(within(drawer()).getByRole("button", { name: /#10/ }));

    expect((await within(drawer()).findByText(/already waits/)).textContent).toBe("#10 already waits on #5");
  });

  it("u17_complete_calls_todo_complete", async () => {
    const { app } = await openDrawerOf(/#5/);
    app.handlers["todo.complete"] = () => blocked();

    fireEvent.click(within(drawer()).getByText("complete"));

    await waitFor(() => expect(callsTo(app, "todo.complete")).toEqual([{ method: "todo.complete", params: { id: 5 } }]));
  });

  it("u17_delete_calls_todo_delete_and_closes_the_drawer", async () => {
    const { app, connected } = await openDrawerOf(/#5/);
    app.handlers["todo.delete"] = () => null;

    fireEvent.click(within(drawer()).getByText("delete"));
    fireEvent.click(within(drawer()).getAllByText("delete")[0]!);

    await waitFor(() => expect(connected.drawer.content()).toBeNull());
    expect(callsTo(app, "todo.delete")).toEqual([{ method: "todo.delete", params: { id: 5 } }]);
  });

  it("u17_a_failed_delete_keeps_the_drawer_open_and_shows_the_message", async () => {
    const { app, connected } = await openDrawerOf(/#5/);
    app.handlers["todo.delete"] = () => Promise.reject(new RpcError(-32001, "not found"));

    fireEvent.click(within(drawer()).getByText("delete"));
    fireEvent.click(within(drawer()).getAllByText("delete")[0]!);

    expect([(await within(drawer()).findByText(/not found/)).textContent, connected.drawer.content() === null]).toEqual([
      "not found",
      false,
    ]);
  });

  it("u17_double_clicking_the_title_edits_it_and_leaving_calls_todo_update_once", async () => {
    const { app } = await openDrawerOf(/#5/);
    app.handlers["todo.update"] = () => blocked();
    fireEvent.dblClick(within(drawer()).getByText("session store"));
    const field = within(drawer()).getByRole("textbox", { name: "title" });

    fireEvent.input(field, { target: { value: "session cache" } });
    fireEvent.blur(field);
    fireEvent.blur(field);

    expect(callsTo(app, "todo.update")).toEqual([{ method: "todo.update", params: { id: 5, title: "session cache", body: null } }]);
  });

  it("u17_enter_in_the_title_field_leaves_it", async () => {
    const { app } = await openDrawerOf(/#5/);
    app.handlers["todo.update"] = () => blocked();
    fireEvent.dblClick(within(drawer()).getByText("session store"));
    const field = within(drawer()).getByRole("textbox", { name: "title" });
    field.focus();

    fireEvent.input(field, { target: { value: "session cache" } });
    fireEvent.keyDown(field, { key: "Enter" });

    await waitFor(() => expect(callsTo(app, "todo.update")).toHaveLength(1));
  });

  it("u17_double_clicking_the_body_edits_it_and_leaving_calls_todo_update_with_the_body", async () => {
    const { app } = await openDrawerOf(/#5/);
    app.handlers["todo.update"] = () => blocked();
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

  it("u17_leaving_a_field_untouched_after_another_actor_renamed_the_todo_calls_nothing_and_shows_their_title", async () => {
    const { app, store } = await openDrawerOf(/#5/);
    fireEvent.dblClick(within(drawer()).getByText("session store"));
    const field = within(drawer()).getByRole("textbox", { name: "title" });
    store.todos = shelf().map((each) => (each.id === 5 ? { ...each, title: "their title" } : each));
    app.emit(todoEvent("todo.updated", todo(5)));
    await screen.findByRole("button", { name: /their title/ });

    fireEvent.blur(field);

    expect(callsTo(app, "todo.update")).toEqual([]);
    expect(await within(drawer()).findByText("their title")).toBeTruthy();
  });

  it("u17_a_failed_update_reopens_the_field_with_the_typed_text", async () => {
    const { app } = await openDrawerOf(/#5/);
    app.handlers["todo.update"] = () => Promise.reject(new RpcError(-32602, "title is empty"));
    fireEvent.dblClick(within(drawer()).getByText("session store"));
    fireEvent.input(within(drawer()).getByRole("textbox", { name: "title" }), { target: { value: "typed" } });

    fireEvent.blur(within(drawer()).getByRole("textbox", { name: "title" }));

    await within(drawer()).findByText(/title is empty/);
    expect(within(drawer()).getByRole("textbox", { name: "title" })).toHaveProperty("value", "typed");
  });

  it("u17_when_another_actor_deletes_the_open_todo_the_drawer_says_so", async () => {
    const { app, store } = await openDrawerOf(/#5/);
    store.todos = shelf().filter((each) => each.id !== 5);

    app.emit({ actor: USER, name: "todo.deleted", data: { id: 5 } });

    expect((await within(drawer()).findByText(/was deleted/)).textContent).toBe("#5 was deleted");
  });

  it("u17_the_last_touch_line_comes_before_complete_and_delete", async () => {
    const { app } = await mountTodos(shelf());
    app.handlers["provenance.history"] = () => [
      { actor: USER, verb: "wrote", item: "todo:5", at: new Date(2026, 0, 5, 13, 10).getTime() },
    ];
    fireEvent.click(await rowButton(/#5/));

    const last = await within(drawer()).findByText(/^last/);

    expect(last.compareDocumentPosition(within(drawer()).getByText("complete")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("u17_two_quick_blocker_picks_both_land_the_second_built_on_the_first", async () => {
    const { app } = await openDrawerOf(/#5/);
    app.handlers["todo.setBlockers"] = async ({ blockers }) => ({ ...blocked(), blockers });
    fireEvent.click(within(drawer()).getByText("blocker"));
    fireEvent.click(within(drawer()).getByRole("button", { name: /#7/ }));
    fireEvent.click(within(drawer()).getByText("blocker"));

    fireEvent.click(within(drawer()).getByRole("button", { name: /#10/ }));

    await waitFor(() => expect(callsTo(app, "todo.setBlockers")).toHaveLength(2));
    expect(callsTo(app, "todo.setBlockers").map((call) => call.params)).toEqual([
      { id: 5, blockers: [4, 7] },
      { id: 5, blockers: [4, 7, 10] },
    ]);
  });

  it("u17_a_blocker_change_by_another_actor_between_two_picks_is_not_undone_by_the_second", async () => {
    const { app, store } = await openDrawerOf(/#5/);
    app.handlers["todo.setBlockers"] = async ({ blockers }) => ({ ...blocked(), blockers });
    fireEvent.click(within(drawer()).getByText("blocker"));
    fireEvent.click(within(drawer()).getByRole("button", { name: /#7/ }));
    await waitFor(() => expect(callsTo(app, "todo.setBlockers")).toHaveLength(1));
    store.todos = shelf().map((each) => (each.id === 5 ? { ...each, blockers: [], blocked: false } : each));
    app.emit(todoEvent("todo.updated", todo(5)));
    await waitFor(() => expect(within(drawer()).queryByText("waits on")).toBeNull());

    fireEvent.click(within(drawer()).getByText("blocker"));
    fireEvent.click(within(drawer()).getByRole("button", { name: /#10/ }));

    await waitFor(() => expect(callsTo(app, "todo.setBlockers")).toHaveLength(2));
    expect(callsTo(app, "todo.setBlockers")[1]?.params).toEqual({ id: 5, blockers: [10] });
  });

  it("u17_a_todo_just_picked_is_not_offered_again_before_the_event_arrives", async () => {
    const { app } = await openDrawerOf(/#5/);
    app.handlers["todo.setBlockers"] = async ({ blockers }) => ({ ...blocked(), blockers });
    fireEvent.click(within(drawer()).getByText("blocker"));
    fireEvent.click(within(drawer()).getByRole("button", { name: /#7/ }));
    await waitFor(() => expect(callsTo(app, "todo.setBlockers")).toHaveLength(1));

    fireEvent.click(within(drawer()).getByText("blocker"));

    expect(within(drawer()).queryByRole("button", { name: /#7/ })).toBeNull();
  });

  it("u17_a_failed_todo_get_offers_no_complete_or_delete", async () => {
    const { app } = await mountTodos(shelf());
    app.handlers["todo.get"] = () => Promise.reject(new RpcError(-32001, "gone"));

    fireEvent.click(await rowButton(/#5/));

    await within(drawer()).findByText(/gone/);
    expect([within(drawer()).queryByText("complete"), within(drawer()).queryByText("delete")]).toEqual([null, null]);
  });

  it("u17_deleting_your_own_todo_while_the_drawer_slides_out_shows_no_deleted_message", async () => {
    const { app, store } = await openDrawerOf(/#5/, shelf(), false);
    app.handlers["todo.delete"] = async () => {
      store.todos = shelf().filter((each) => each.id !== 5);

      return null;
    };

    fireEvent.click(within(drawer()).getByText("delete"));
    fireEvent.click(within(drawer()).getAllByText("delete")[0]!);
    app.emit({ actor: USER, name: "todo.deleted", data: { id: 5 } });
    await waitFor(() => expect(screen.queryByRole("button", { name: /#5/ })).toBeNull());

    expect(within(drawer()).queryByText(/was deleted/)).toBeNull();
  });

  it("u17_a_pick_that_fails_with_a_bug_is_reported_and_the_next_pick_still_runs", async () => {
    const { app } = await openDrawerOf(/#5/);
    const reportError = vi.fn();

    vi.stubGlobal("reportError", reportError);
    app.handlers["todo.setBlockers"] = (() => {
      let calls = 0;

      return async ({ blockers }) => {
        calls += 1;

        if (calls === 1) throw "boom";

        return { ...blocked(), blockers };
      };
    })();
    fireEvent.click(within(drawer()).getByText("blocker"));
    fireEvent.click(within(drawer()).getByRole("button", { name: /#7/ }));
    fireEvent.click(within(drawer()).getByText("blocker"));
    fireEvent.click(within(drawer()).getByRole("button", { name: /#10/ }));

    await waitFor(() => expect(callsTo(app, "todo.setBlockers")).toHaveLength(2));
    expect(reportError).toHaveBeenCalledWith("boom");
  });

  it("u17_rowButton_resolves_the_row_not_a_blockers_link_when_the_link_renders_first", async () => {
    await mountTodos([todo(7, { title: "waits later", blocked: true, blockers: [9] }), todo(9, { title: "later todo" })]);
    await screen.findByText(/^waits on/);

    const row = await rowButton(/#9/);

    expect(row.className).not.toBe("word");
  });

  it("u50_the_blocker_button_sits_in_a_paragraph_so_the_drawers_flex_column_does_not_stretch_it", async () => {
    await openDrawerOf(/#5/);

    expect(within(drawer()).getByText("blocker").parentElement?.tagName).toBe("P");
  });
});
