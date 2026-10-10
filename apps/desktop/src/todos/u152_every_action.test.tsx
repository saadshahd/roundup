import { cleanup, fireEvent, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { RpcError } from "../app/seam";
import { mountTodos, todo, todoCallsTo as callsTo, todoEvent, todoRowOf as rowOf } from "./testHarness";

afterEach(cleanup);

type Mounted = Awaited<ReturnType<typeof mountTodos>>;

const drawer = () => screen.getByRole("complementary", { name: "drawer" });

/** The Event the Daemon sends after a write: the list refetches from the store. */
const refetch = ({ app, store }: Mounted) => app.emit(todoEvent("todo.updated", { ...store.todos[0]! }));

/** A pointer click, or the keyboard: focus the target and press it, which a browser turns into a click on a button (Enter or Space). */
const press = (target: HTMLElement, by: "pointer" | "keyboard") => {
  if (by === "keyboard") {
    target.focus();
    expect(document.activeElement).toBe(target);
  }

  fireEvent.click(target);
};

const rowText = (id: number) => rowOf(id).textContent;

const openDrawerOf = async (id: number) => {
  fireEvent.click(await screen.findByRole("button", { name: new RegExp(`^(idle|blocked) #${id} `) }));
  await within(drawer()).findByText("blocker");
};

for (const by of ["pointer", "keyboard"] as const) {
  describe(`u152 every Todo action by the ${by}`, () => {
    it(`u152_add_${by}`, async () => {
      const mounted = await mountTodos([todo(1)]);
      mounted.app.handlers["todo.create"] = ({ title }) => {
        mounted.store.todos = [...mounted.store.todos, todo(2, { title })];

        return mounted.store.todos[1]!;
      };

      press(await screen.findByRole("button", { name: "add todo" }), by);

      const field = screen.getByRole("textbox", { name: "new todo title" });

      fireEvent.input(field, { target: { value: "ship it" } });
      fireEvent.keyDown(field, { key: "Enter" });
      await waitFor(() => expect(callsTo(mounted.app, "todo.create")).toEqual([{ method: "todo.create", params: { title: "ship it", body: null, blockers: null } }]));
      refetch(mounted);

      await waitFor(() => expect(rowText(2)).toContain("ship it"));
    });

    it(`u152_read_${by}`, async () => {
      await mountTodos([todo(1), todo(2)]);

      press(await screen.findByRole("button", { name: /^idle #2 / }), by);

      expect(await within(drawer()).findByText("blocker")).toBeTruthy();
    });

    it(`u152_edit_${by}`, async () => {
      const mounted = await mountTodos([todo(1), todo(2)]);

      await openDrawerOf(2);

      const title = within(drawer()).getByText("todo 2");

      if (by === "pointer") fireEvent.dblClick(title);
      else {
        title.focus();
        fireEvent.keyDown(title, { key: "Enter" });
      }

      const field = await within(drawer()).findByLabelText("title");

      fireEvent.input(field, { target: { value: "renamed" } });
      fireEvent.change(field, { target: { value: "renamed" } });
      fireEvent.blur(field);
      await waitFor(() => expect(callsTo(mounted.app, "todo.update")).toEqual([{ method: "todo.update", params: { id: 2, title: "renamed", body: null } }]));
      refetch(mounted);

      await waitFor(() => expect(rowText(2)).toContain("renamed"));
    });

    it(`u152_block_${by}`, async () => {
      const mounted = await mountTodos([todo(1), todo(2)]);

      await openDrawerOf(2);
      press(within(drawer()).getByRole("button", { name: /blocker/ }), by);
      press(await within(drawer()).findByRole("button", { name: /#1 / }), by);
      await waitFor(() => expect(callsTo(mounted.app, "todo.setBlockers")).toEqual([{ method: "todo.setBlockers", params: { id: 2, blockers: [1] } }]));
      refetch(mounted);

      await waitFor(() => expect(rowOf(2).querySelector("[data-todo-waits]")?.textContent).toBe("waits on #1"));
    });

    it(`u152_unblock_${by}`, async () => {
      const mounted = await mountTodos([todo(1), todo(3), todo(2, { blocked: true, blockers: [1, 3] })]);

      await openDrawerOf(2);
      press(within(drawer()).getByRole("button", { name: "remove blocker #1" }), by);
      await waitFor(() => expect(callsTo(mounted.app, "todo.setBlockers")).toEqual([{ method: "todo.setBlockers", params: { id: 2, blockers: [3] } }]));
      refetch(mounted);
      await waitFor(() => expect(rowOf(2).querySelector("[data-todo-waits]")?.textContent).toBe("waits on #3"));

      press(within(drawer()).getByRole("button", { name: "remove blocker #3" }), by);
      await waitFor(() => expect(callsTo(mounted.app, "todo.setBlockers")[1]?.params).toEqual({ id: 2, blockers: [] }));
      refetch(mounted);

      await waitFor(() => expect(rowOf(2).querySelector("[data-todo-waits]")).toBeNull());
    });

    it(`u152_complete_from_the_row_${by}`, async () => {
      const mounted = await mountTodos([todo(1), todo(2)]);

      await screen.findByRole("button", { name: /^idle #2 / });

      if (by === "pointer") fireEvent.mouseEnter(rowOf(2));
      else (await screen.findByRole("button", { name: /^idle #2 / })).focus();

      press(await screen.findByRole("button", { name: "complete" }), by);
      await waitFor(() => expect(callsTo(mounted.app, "todo.complete")).toEqual([{ method: "todo.complete", params: { id: 2 } }]));
      refetch(mounted);

      await screen.findByRole("button", { name: "1 done" });
      expect(document.querySelector('[data-id="2"]')).toBeNull();
    });

    it(`u152_complete_from_the_drawer_${by}`, async () => {
      const mounted = await mountTodos([todo(1), todo(2)]);

      await openDrawerOf(2);
      press(await within(drawer()).findByRole("button", { name: "complete" }), by);
      await waitFor(() => expect(callsTo(mounted.app, "todo.complete")).toEqual([{ method: "todo.complete", params: { id: 2 } }]));
    });

    it(`u152_delete_${by}`, async () => {
      const mounted = await mountTodos([todo(1), todo(2)]);

      await openDrawerOf(2);
      press(await within(drawer()).findByRole("button", { name: "delete" }), by);
      press(await within(drawer()).findByRole("button", { name: "delete" }), by);
      await waitFor(() => expect(callsTo(mounted.app, "todo.delete")).toEqual([{ method: "todo.delete", params: { id: 2 } }]));
      mounted.app.emit(todoEvent("todo.updated", { ...mounted.store.todos[0]! }));

      await waitFor(() => expect(document.querySelector('[data-id="2"]')).toBeNull());
    });
  });
}

describe("u152 focus stays in the Drawer", () => {
  it("u152_choosing_an_offered_blocker_by_keyboard_leaves_focus_off_the_body", async () => {
    await mountTodos([todo(1), todo(2)]);
    await openDrawerOf(2);
    press(within(drawer()).getByRole("button", { name: /blocker/ }), "keyboard");
    press(await within(drawer()).findByRole("button", { name: /#1 / }), "keyboard");

    await waitFor(() => expect(document.activeElement).not.toBe(document.body));
    expect(drawer().contains(document.activeElement)).toBe(true);
  });

  it("u152_remove_by_keyboard_leaves_focus_off_the_body", async () => {
    const mounted = await mountTodos([todo(1), todo(3), todo(2, { blocked: true, blockers: [1, 3] })]);

    await openDrawerOf(2);
    press(within(drawer()).getByRole("button", { name: "remove blocker #1" }), "keyboard");
    await waitFor(() => expect(callsTo(mounted.app, "todo.setBlockers")).toHaveLength(1));
    mounted.store.todos = mounted.store.todos.map((t) => (t.id === 2 ? { ...t, blockers: [3] } : t));
    refetch(mounted);

    await waitFor(() => expect(rowOf(2).querySelector("[data-todo-waits]")?.textContent).toBe("waits on #3"));
    expect(document.activeElement).not.toBe(document.body);
    expect(drawer().contains(document.activeElement)).toBe(true);
  });
});

describe("u152 remove a blocker", () => {
  it("u152_remove_names_each_blocker_done_or_open_and_keeps_the_waits_on_listing", async () => {
    await mountTodos([todo(1, { done: true }), todo(3), todo(2, { blocked: true, blockers: [1, 3] })]);

    await openDrawerOf(2);

    for (const id of [1, 3]) expect(within(drawer()).getByRole("button", { name: `remove blocker #${id}` }).textContent).toBe("remove");
    expect(within(drawer()).getByText("waits on").parentElement?.children).toHaveLength(5);
  });

  it("u152_a_failed_remove_shows_the_message_and_keeps_the_row", async () => {
    const mounted = await mountTodos([todo(1), todo(2, { blocked: true, blockers: [1] })]);

    mounted.app.handlers["todo.setBlockers"] = () => Promise.reject(new RpcError(-32010, "cannot remove"));
    await openDrawerOf(2);
    fireEvent.click(within(drawer()).getByRole("button", { name: "remove blocker #1" }));

    expect((await within(drawer()).findByText(/cannot remove/)).textContent).toBe("cannot remove");
    expect(rowOf(2).querySelector("[data-todo-waits]")?.textContent).toBe("waits on #1");
  });
});
