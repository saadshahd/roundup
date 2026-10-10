import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { agent, room } from "../testing/nodes";
import { mountTodos, todo } from "./testHarness";

afterEach(cleanup);

const seed = () => [todo(1, { home: "r1" }), todo(2, { home: null }), todo(3, { home: "r2" })];

const rail = () => [room("r1", { name: "alpha" }), room("r2", { name: "beta" })];

const ids = () => [...document.querySelectorAll("[data-shelf-row]")].map((row) => row.getAttribute("data-id"));

describe("u159 the Shelf can show only the selected Room's Todos", () => {
  it("u159_the_pair_shows_only_with_a_room_selected", async () => {
    const { connected } = await mountTodos(seed(), true, [...rail(), agent("a1", "idle", "ann")]);

    await screen.findByText(/todo 1/);
    expect(screen.queryByRole("radio", { name: "this room" })).toBeNull();

    connected.rail.select("a1");
    await Promise.resolve();
    expect(screen.queryByRole("radio", { name: "this room" })).toBeNull();

    connected.rail.select("r1");
    expect(await screen.findByRole("radio", { name: "this room" })).toBeTruthy();
    expect(screen.getByRole("radio", { name: "all" })).toHaveProperty("checked", true);
    expect(ids()).toEqual(["1", "2", "3"]);
  });

  it("u159_this_room_hides_the_root_and_another_rooms_todo", async () => {
    const { connected } = await mountTodos(seed(), true, rail());

    await screen.findByText(/todo 1/);
    connected.rail.select("r1");
    fireEvent.click(await screen.findByRole("radio", { name: "this room" }));

    await waitFor(() => expect(ids()).toEqual(["1"]));
  });

  it("u159_selecting_another_room_resets_to_all", async () => {
    const { connected } = await mountTodos(seed(), true, rail());

    await screen.findByText(/todo 1/);
    connected.rail.select("r1");
    fireEvent.click(await screen.findByRole("radio", { name: "this room" }));
    await waitFor(() => expect(ids()).toEqual(["1"]));

    connected.rail.select("r2");

    await waitFor(() => expect(ids()).toEqual(["1", "2", "3"]));
    expect(screen.getByRole("radio", { name: "all" })).toHaveProperty("checked", true);
  });

  it("u159_returning_to_a_room_reads_all", async () => {
    const { connected } = await mountTodos(seed(), true, rail());

    await screen.findByText(/todo 1/);
    connected.rail.select("r1");
    fireEvent.click(await screen.findByRole("radio", { name: "this room" }));
    await waitFor(() => expect(ids()).toEqual(["1"]));

    connected.rail.select("r2");
    connected.rail.select("r1");

    await waitFor(() => expect(ids()).toEqual(["1", "2", "3"]));
    expect(screen.getByRole("radio", { name: "all" })).toHaveProperty("checked", true);
  });

  it("u159_an_empty_result_reads_no_todos_in_this_room", async () => {
    const { connected } = await mountTodos([todo(2, { home: null })], true, rail());

    await screen.findByText(/todo 2/);
    connected.rail.select("r1");
    fireEvent.click(await screen.findByRole("radio", { name: "this room" }));

    expect(await screen.findByText("no todos in this room")).toBeTruthy();
    expect(screen.queryByText("no todos yet")).toBeNull();
  });
});
