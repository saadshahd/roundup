import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { agent, workstream } from "../testing/nodes";
import { mountTodos, todo } from "./testHarness";

afterEach(cleanup);

const seed = () => [todo(1, { home: "r1" }), todo(2, { home: null }), todo(3, { home: "r2" })];

const rail = () => [workstream("r1", { name: "alpha" }), workstream("r2", { name: "beta" })];

const ids = () => [...document.querySelectorAll("[data-shelf-row]")].map((row) => row.getAttribute("data-id"));

describe("u159 the Shelf can show only the selected Workstream's Todos", () => {
  it("u159_the_pair_shows_only_with_a_workstream_selected", async () => {
    const { connected } = await mountTodos(seed(), true, [...rail(), agent("a1", "idle", "ann")]);

    await screen.findByText(/todo 1/);
    expect(screen.queryByRole("radio", { name: "this workstream" })).toBeNull();

    connected.rail.select("a1");
    await Promise.resolve();
    expect(screen.queryByRole("radio", { name: "this workstream" })).toBeNull();

    connected.rail.select("r1");
    expect(await screen.findByRole("radio", { name: "this workstream" })).toBeTruthy();
    expect(screen.getByRole("radio", { name: "all" })).toHaveProperty("checked", true);
    expect(ids()).toEqual(["1", "2", "3"]);
  });

  it("u159_this_workstream_hides_the_root_and_another_workstreams_todo", async () => {
    const { connected } = await mountTodos(seed(), true, rail());

    await screen.findByText(/todo 1/);
    connected.rail.select("r1");
    fireEvent.click(await screen.findByRole("radio", { name: "this workstream" }));

    await waitFor(() => expect(ids()).toEqual(["1"]));
  });

  it("u159_selecting_another_workstream_resets_to_all", async () => {
    const { connected } = await mountTodos(seed(), true, rail());

    await screen.findByText(/todo 1/);
    connected.rail.select("r1");
    fireEvent.click(await screen.findByRole("radio", { name: "this workstream" }));
    await waitFor(() => expect(ids()).toEqual(["1"]));

    connected.rail.select("r2");

    await waitFor(() => expect(ids()).toEqual(["1", "2", "3"]));
    expect(screen.getByRole("radio", { name: "all" })).toHaveProperty("checked", true);
  });

  it("u159_returning_to_a_workstream_reads_all", async () => {
    const { connected } = await mountTodos(seed(), true, rail());

    await screen.findByText(/todo 1/);
    connected.rail.select("r1");
    fireEvent.click(await screen.findByRole("radio", { name: "this workstream" }));
    await waitFor(() => expect(ids()).toEqual(["1"]));

    connected.rail.select("r2");
    connected.rail.select("r1");

    await waitFor(() => expect(ids()).toEqual(["1", "2", "3"]));
    expect(screen.getByRole("radio", { name: "all" })).toHaveProperty("checked", true);
  });

  it("u159_an_empty_result_reads_no_todos_in_this_workstream", async () => {
    const { connected } = await mountTodos([todo(2, { home: null })], true, rail());

    await screen.findByText(/todo 2/);
    connected.rail.select("r1");
    fireEvent.click(await screen.findByRole("radio", { name: "this workstream" }));

    expect(await screen.findByText("no todos in this workstream")).toBeTruthy();
    expect(screen.queryByText("no todos yet")).toBeNull();
  });

  it("u159_an_empty_project_reads_only_the_workstream_line", async () => {
    const { connected } = await mountTodos([], true, rail());

    await screen.findByText("no todos yet");
    connected.rail.select("r1");
    fireEvent.click(await screen.findByRole("radio", { name: "this workstream" }));

    expect(await screen.findByText("no todos in this workstream")).toBeTruthy();
    expect(screen.queryByText("no todos yet")).toBeNull();
  });
});
