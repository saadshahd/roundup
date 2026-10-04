import { cleanup, fireEvent, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { assertKind, mountTodos, todo, todoCallsTo as callsTo, todoEvent, todoRowOf } from "./testHarness";
import { USER } from "../testing/nodes";

afterEach(cleanup);

const rows = () => within(screen.getByRole("region", { name: "todos" })).getAllByRole("button").map((row) => row.textContent?.trim());

describe("u15 Todo list", () => {
  it("u15_open_todos_are_listed_in_id_order_as_glyph_hash_id_title", async () => {
    await mountTodos([todo(7, { title: "split checkout flow" }), todo(3, { title: "refresh tokens" })]);

    await screen.findByText(/refresh tokens/);

    expect(rows()).toEqual(["", "#3 refresh tokens", "#7 split checkout flow"]);
    assertKind(screen.getByRole("button", { name: "idle #3 refresh tokens" }), "idle", "dot");
  });

  it("u15_a_blocked_todo_has_the_pause_glyph_and_a_second_line_naming_its_open_blockers_in_id_order", async () => {
    await mountTodos([
      todo(5, { title: "session store", blocked: true, blockers: [6, 4] }),
      todo(6, { title: "later" }),
      todo(4, { title: "migrate" }),
    ]);

    await screen.findByText(/session store/);

    assertKind(screen.getByRole("button", { name: "blocked #5 session store" }), "blocked", "pause");
    expect([rows()[2], screen.getByText(/^waits on/).textContent]).toEqual(["#5 session store", "waits on #4, #6"]);
  });

  it("u15_a_done_blocker_is_not_named_on_the_blocked_todos_second_line", async () => {
    await mountTodos([
      todo(5, { title: "session store", blocked: true, blockers: [4, 6] }),
      todo(4, { done: true }),
      todo(6),
    ]);

    await screen.findByText(/session store/);

    expect(screen.getByText(/^waits on/).textContent).toBe("waits on #6");
  });

  it("u15_done_todos_fold_into_one_line_counting_them", async () => {
    await mountTodos([todo(1, { done: true, title: "old one" }), todo(2, { done: true }), todo(3)]);

    const summary = await screen.findByRole("button", { name: "2 done" });

    expect(summary.querySelectorAll("svg")).toHaveLength(1);
    expect(summary.querySelector("svg.lucide-check")?.getAttribute("aria-hidden")).toBe("true");

    expect([rows(), screen.queryByText(/old one/)]).toEqual([["", "#3 todo 3", "2 done"], null]);
  });

  it("u15_clicking_the_done_line_unfolds_the_done_todos", async () => {
    await mountTodos([todo(1, { done: true, title: "old one" })]);

    fireEvent.click(await screen.findByRole("button", { name: "1 done" }));

    expect(rows()).toEqual(["", "1 done", "#1 old one"]);
    assertKind(screen.getByRole("button", { name: "done #1 old one" }), "done", "check");
  });

  it("u15_no_done_todos_show_no_done_line", async () => {
    await mountTodos([todo(1)]);
    await screen.findByText(/todo 1/);

    expect(screen.queryByText(/done/)).toBeNull();
  });

  it.each(["todo.created", "todo.updated"] as const)("u15_%s_refetches_todo_list", async (name) => {
    const { app, store } = await mountTodos([todo(1)]);
    await screen.findByText(/todo 1/);
    store.todos = [todo(1), todo(2, { title: "fresh" })];

    app.emit(todoEvent(name, todo(2)));

    expect(await screen.findByText(/fresh/)).toBeTruthy();
  });

  it.each(["todo.unblocked", "todo.deleted"] as const)("u15_%s_refetches_todo_list", async (name) => {
    const { app, store } = await mountTodos([todo(1), todo(2)]);
    await screen.findByText(/todo 2/);
    store.todos = [todo(2)];

    app.emit({ actor: USER, name, data: { id: 1 } });

    await waitFor(() => expect(screen.queryByText(/todo 1/)).toBeNull());
  });

  it("u15_an_event_that_is_not_about_a_todo_does_not_refetch", async () => {
    const { app } = await mountTodos([todo(1)]);
    await screen.findByText(/todo 1/);

    app.emit({ actor: USER, name: "rail.changed" });
    app.emit({ actor: USER, name: "pad.changed", data: { name: "notes" } });

    expect(callsTo(app, "todo.list")).toHaveLength(1);
  });

  it("u15_the_list_never_calls_todo_get", async () => {
    const { app, store } = await mountTodos([todo(1)]);
    await screen.findByText(/todo 1/);
    store.todos = [todo(1, { title: "changed" })];

    app.emit(todoEvent("todo.updated", todo(1)));
    await screen.findByText(/changed/);

    expect(callsTo(app, "todo.get")).toEqual([]);
  });

  it("u15_a_slow_older_answer_never_overwrites_a_newer_one", async () => {
    const { app, store } = await mountTodos([todo(1)]);
    await screen.findByText(/todo 1/);
    let releaseOlder = () => {};

    const older = new Promise<void>((release) => {
      releaseOlder = release;
    });

    app.handlers["todo.list"] = async () => {
      await older;

      return [todo(1, { title: "stale" })];
    };

    app.emit(todoEvent("todo.updated", todo(1)));
    app.handlers["todo.list"] = () => store.todos;
    store.todos = [todo(1, { title: "newest" })];
    app.emit(todoEvent("todo.updated", todo(1)));

    await screen.findByText(/newest/);

    releaseOlder();
    await older;
    await new Promise((settle) => setTimeout(settle));

    expect(screen.queryByText(/stale/)).toBeNull();
  });

  it("u15_a_failed_todo_list_shows_its_message_in_ink", async () => {
    const { app } = await mountTodos([]);
    app.handlers["todo.list"] = () => Promise.reject(new Error("daemon says no"));

    app.emit(todoEvent("todo.updated", todo(1)));

    expect((await screen.findByText(/daemon says no/)).textContent).toBe("daemon says no");
  });

  it("u15_a_refetch_that_changes_one_todo_leaves_every_other_row_in_place", async () => {
    const { app, store } = await mountTodos([todo(1), todo(2)]);
    const before = (await screen.findByRole("button", { name: /#2/ }));
    store.todos = [todo(1, { title: "renamed" }), todo(2)];

    app.emit(todoEvent("todo.updated", todo(1)));

    await screen.findByRole("button", { name: /renamed/ });
    expect(screen.getByRole("button", { name: /#2/ })).toBe(before);
  });

  it("u15_listing_blocked_todos_does_a_constant_amount_of_work_per_todo", async () => {
    const count = 120;
    let reads = 0;

    const counting = <Value,>(value: Value) => ({
      enumerable: true,
      get: () => {
        reads += 1;

        return value;
      },
    });

    const counted = (id: number) => {
      const row = todo(id, { blocked: true });

      Object.defineProperty(row, "id", counting(id));

      return Object.defineProperty(row, "blockers", counting([(id % count) + 1]));
    };

    await mountTodos(Array.from({ length: count }, (_, index) => counted(index + 1)));
    await waitFor(() => expect(within(todoRowOf(count)).getByRole("button", { name: /#120 todo 120/ })).toBeTruthy());

    expect(reads).toBeLessThan(count * 40);
  });

  it("u15_a_row_wraps_under_its_id_not_under_its_glyph", async () => {
    await mountTodos([todo(3)]);

    const row = await screen.findByRole("button", { name: /#3/ });

    expect([row.style.paddingLeft, row.style.textIndent]).toEqual(["var(--todo-row-indent)", "calc(-1 * var(--todo-row-indent))"]);
  });

  it("u15_a_list_that_is_gone_stops_listening_to_events", async () => {
    const { app, unmount } = await mountTodos([todo(1)]);
    await screen.findByText(/todo 1/);
    unmount();

    app.emit(todoEvent("todo.updated", todo(1)));

    expect(callsTo(app, "todo.list")).toHaveLength(1);
  });
});
