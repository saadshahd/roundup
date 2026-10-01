import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { RpcError } from "../app/seam";
import { callsTo, mountTodos, todo, todoEvent } from "./testHarness";

afterEach(cleanup);

const openField = async () => {
  fireEvent.click(await screen.findByText("+"));

  return screen.getByRole("textbox", { name: "new todo title" });
};

const type = (field: HTMLElement, text: string) => fireEvent.input(field, { target: { value: text } });

describe("u16 create a Todo", () => {
  it("u16_clicking_plus_opens_one_inline_title_field_at_the_top_of_the_list", async () => {
    await mountTodos([todo(1)]);

    const field = await openField();

    expect(field.nextElementSibling?.textContent).toBe("· #1 todo 1");
  });

  it("u16_clicking_plus_twice_still_shows_one_field", async () => {
    await mountTodos([]);
    await openField();

    fireEvent.click(screen.getByText("+"));

    expect(screen.getAllByRole("textbox")).toHaveLength(1);
  });

  it("u16_clicking_plus_while_typing_keeps_the_typed_text", async () => {
    await mountTodos([]);
    type(await openField(), "half a thought");

    fireEvent.click(screen.getByText("+"));

    expect(screen.getByRole("textbox", { name: "new todo title" })).toHaveProperty("value", "half a thought");
  });

  it("u16_enter_calls_todo_create_with_the_title_and_closes_the_field", async () => {
    const { app } = await mountTodos([]);
    app.handlers["todo.create"] = ({ title }) => todo(1, { title });
    const field = await openField();
    type(field, "  rate limits ");

    fireEvent.keyDown(field, { key: "Enter" });

    await waitFor(() => expect(callsTo(app, "todo.create")).toEqual([
      { method: "todo.create", params: { title: "rate limits", body: null, blockers: null } },
    ]));
    expect(screen.queryByRole("textbox")).toBeNull();
  });

  it("u16_the_new_row_appears_through_the_event_not_the_call", async () => {
    const { app, store } = await mountTodos([]);
    app.handlers["todo.create"] = ({ title }) => todo(1, { title });
    const field = await openField();
    type(field, "rate limits");
    fireEvent.keyDown(field, { key: "Enter" });
    await waitFor(() => expect(callsTo(app, "todo.create")).toHaveLength(1));
    const before = screen.queryByText(/rate limits/);

    store.todos = [todo(1, { title: "rate limits" })];
    app.emit(todoEvent("todo.created", todo(1)));

    expect([before, await screen.findByText(/rate limits/)].map(Boolean)).toEqual([false, true]);
  });

  it("u16_escape_closes_the_field_and_calls_nothing", async () => {
    const { app } = await mountTodos([]);
    const field = await openField();
    type(field, "half a thought");

    fireEvent.keyDown(field, { key: "Escape" });

    expect([screen.queryByRole("textbox"), callsTo(app, "todo.create")]).toEqual([null, []]);
  });

  it.each(["", "   "])("u16_enter_on_the_empty_title_%j_closes_the_field_and_calls_nothing", async (text) => {
    const { app } = await mountTodos([]);
    const field = await openField();
    type(field, text);

    fireEvent.keyDown(field, { key: "Enter" });

    expect([screen.queryByRole("textbox"), callsTo(app, "todo.create")]).toEqual([null, []]);
  });

  it("u16_a_failed_create_shows_the_message_in_place_of_the_field", async () => {
    const { app } = await mountTodos([]);
    app.handlers["todo.create"] = () => Promise.reject(new RpcError(-32602, "title is empty"));
    const field = await openField();
    type(field, "x");

    fireEvent.keyDown(field, { key: "Enter" });

    expect([(await screen.findByText(/title is empty/)).textContent, screen.queryByRole("textbox")]).toEqual([
      "✕ title is empty",
      null,
    ]);
  });

  it("u16_plus_after_a_failure_replaces_the_message_with_a_fresh_field", async () => {
    const { app } = await mountTodos([]);
    app.handlers["todo.create"] = () => Promise.reject(new RpcError(-32602, "title is empty"));
    const field = await openField();
    type(field, "x");
    fireEvent.keyDown(field, { key: "Enter" });
    await screen.findByText(/title is empty/);

    fireEvent.click(screen.getByText("+"));

    expect([screen.queryByText(/title is empty/), screen.getAllByRole("textbox")]).toEqual([null, [expect.anything()]]);
  });
});
