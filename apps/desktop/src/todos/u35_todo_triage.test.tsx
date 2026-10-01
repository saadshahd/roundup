import { cleanup, fireEvent, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { RpcError } from "../app/seam";
import { callsTo, mountTodos, rowOf, todo } from "./testHarness";

afterEach(cleanup);

const drawer = () => screen.getByRole("complementary", { name: "drawer" });

const blocked = () => todo(5, { title: "session store", blocked: true, blockers: [4, 6] });

const shelf = () => [blocked(), todo(4, { title: "migrate" }), todo(6, { title: "later" })];

describe("u35 Todo triage without the Drawer", () => {
  it("u35_hovering_an_open_row_reveals_complete_which_calls_todo_complete", async () => {
    const { app } = await mountTodos([todo(3)]);
    await screen.findByText(/todo 3/);

    fireEvent.mouseEnter(rowOf(3));
    fireEvent.click(screen.getByText("complete"));

    await waitFor(() => expect(callsTo(app, "todo.complete")).toEqual([{ method: "todo.complete", params: { id: 3 } }]));
  });

  it("u35_complete_is_hidden_again_once_the_row_is_no_longer_hovered", async () => {
    await mountTodos([todo(3)]);
    await screen.findByText(/todo 3/);
    fireEvent.mouseEnter(rowOf(3));
    await screen.findByText("complete");

    fireEvent.mouseLeave(rowOf(3));

    expect(screen.queryByText("complete")).toBeNull();
  });

  it("u35_focusing_an_open_row_reveals_complete", async () => {
    await mountTodos([todo(3)]);
    const row = await screen.findByRole("button", { name: /#3/ });

    fireEvent.focusIn(row);

    expect(screen.getByText("complete")).toBeTruthy();
  });

  it("u35_complete_is_hidden_again_once_focus_leaves_the_row", async () => {
    await mountTodos([todo(3)]);
    const row = await screen.findByRole("button", { name: /#3/ });
    fireEvent.focusIn(row);
    await screen.findByText("complete");

    fireEvent.focusOut(row);

    expect(screen.queryByText("complete")).toBeNull();
  });

  it("u35_complete_logs_no_read_touch_and_does_not_open_the_drawer", async () => {
    const { app, connected } = await mountTodos([todo(3)]);
    await screen.findByText(/todo 3/);
    fireEvent.mouseEnter(rowOf(3));

    fireEvent.click(screen.getByText("complete"));
    await waitFor(() => expect(callsTo(app, "todo.complete")).toHaveLength(1));

    expect([callsTo(app, "todo.get"), connected.drawer.content()]).toEqual([[], null]);
  });

  it("u35_a_blockers_id_opens_that_blockers_drawer_and_not_the_blocked_todos_own_drawer", async () => {
    await mountTodos(shelf());
    await screen.findByText(/^waits on/);

    fireEvent.click(screen.getByText("#4"));

    await within(drawer()).findByText("+ blocker");
    // #4 has no blockers of its own; a "waits on" section would mean #5's drawer opened instead.
    expect([
      within(drawer()).queryByText("waits on"),
      within(drawer()).getByText("blocks").nextElementSibling?.textContent,
    ]).toEqual([null, "⏸ #5  session store"]);
  });

  it("u35_a_failed_complete_shows_its_message_in_place_of_the_second_line", async () => {
    const { app } = await mountTodos(shelf());
    await screen.findByText(/^waits on/);
    app.handlers["todo.complete"] = () => Promise.reject(new RpcError(-32000, "cannot complete"));

    fireEvent.mouseEnter(rowOf(5));
    fireEvent.click(screen.getByText("complete"));

    expect((await screen.findByText(/cannot complete/)).textContent).toBe("✕ cannot complete");
    expect(screen.queryByText(/^waits on/)).toBeNull();
  });

  it("u35_a_failed_completes_message_clears_on_the_next_click", async () => {
    const { app } = await mountTodos(shelf());
    await screen.findByText(/^waits on/);
    app.handlers["todo.complete"] = () => Promise.reject(new RpcError(-32000, "cannot complete"));
    fireEvent.mouseEnter(rowOf(5));
    fireEvent.click(screen.getByText("complete"));
    await screen.findByText(/cannot complete/);
    app.handlers["todo.complete"] = () => new Promise(() => {});

    fireEvent.click(screen.getByText("complete"));

    expect(screen.queryByText(/cannot complete/)).toBeNull();
  });

  it("u35_an_open_rows_text_at_rest_never_includes_complete", async () => {
    await mountTodos([todo(3)]);

    expect(await screen.findByRole("button", { name: "· #3 todo 3" })).toBeTruthy();
  });
});
