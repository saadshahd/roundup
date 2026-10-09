import { describe, expect, it } from "vitest";
import type { EventData } from "@contracts/EventData";
import type { Pad } from "@contracts/pad/Pad";
import type { Todo } from "@contracts/todo/Todo";
import { createFakeApp } from "./fakeApp";
import { USER } from "./nodes";
import { padHandlers, todoHandlers } from "./stores";

const todo = (id: number, over: Partial<Todo> = {}): Todo => ({
  id,
  title: `todo ${id}`,
  body: "",
  done: false,
  blockers: [],
  blocked: false,
  created_at: 0,
  creator: USER,
  ...over,
});

const pad = (name: string, text = ""): Pad => ({ name, owner: USER, text, updated_at: 0 });

const todos = (initial: Todo[]) => {
  const announced: EventData[] = [];
  const app = createFakeApp();
  Object.assign(app.handlers, todoHandlers({ todos: initial }, (data) => announced.push(data)));

  return { app, announced };
};

const pads = (initial: Pad[]) => {
  const announced: EventData[] = [];
  const app = createFakeApp();
  Object.assign(app.handlers, padHandlers({ pads: initial, history: [] }, (data) => announced.push(data)));

  return { app, announced };
};

describe("u26 the in-memory Todos", () => {
  it("u26_creating_a_todo_numbers_it_after_the_last_and_sends_todo_created", async () => {
    const { app, announced } = todos([todo(4)]);

    const created = await app.rpc("todo.create", { title: "new", body: null, blockers: null });

    expect([created.id, announced]).toEqual([5, [{ name: "todo.created", data: created }]]);
  });

  it("u26_creating_a_todo_with_an_open_blocker_makes_it_blocked", async () => {
    const { app } = todos([todo(1)]);

    const created = await app.rpc("todo.create", { title: "new", body: null, blockers: [1] });

    expect(created.blocked).toBe(true);
  });

  it("u26_updating_a_todo_changes_the_given_fields_and_sends_todo_updated", async () => {
    const { app, announced } = todos([todo(1, { body: "keep" })]);

    const updated = await app.rpc("todo.update", { id: 1, title: "renamed", body: null });

    expect([updated.title, updated.body, announced]).toEqual(["renamed", "keep", [{ name: "todo.updated", data: updated }]]);
  });

  it("u26_completing_a_blocker_unblocks_the_todo_and_sends_todo_unblocked", async () => {
    const { app, announced } = todos([todo(1), todo(2, { blockers: [1], blocked: true })]);

    await app.rpc("todo.complete", { id: 1 });

    expect([(await app.rpc("todo.get", { id: 2 })).blocked, announced.map((data) => data.name)]).toEqual([
      false,
      ["todo.updated", "todo.unblocked"],
    ]);
  });

  it("u26_setting_blockers_replaces_the_list_and_derives_blocked", async () => {
    const { app } = todos([todo(1), todo(2)]);

    const updated = await app.rpc("todo.setBlockers", { id: 2, blockers: [1] });

    expect([updated.blockers, updated.blocked]).toEqual([[1], true]);
  });

  it("u26_deleting_a_todo_removes_it_from_the_list_and_sends_todo_deleted", async () => {
    const { app, announced } = todos([todo(1), todo(2)]);

    await app.rpc("todo.delete", { id: 1 });

    expect([(await app.rpc("todo.list", null)).map((each) => each.id), announced]).toEqual([[2], [{ name: "todo.deleted", data: { id: 1 } }]]);
  });

  it("u26_a_todo_that_does_not_exist_fails_with_not_found", async () => {
    const { app } = todos([]);

    await expect(app.rpc("todo.get", { id: 9 })).rejects.toMatchObject({ code: -32001 });
  });
});

describe("u26 the in-memory Pads", () => {
  it("u26_creating_a_pad_makes_the_user_its_owner_and_sends_pad_changed", async () => {
    const { app, announced } = pads([]);

    const created = await app.rpc("pad.create", { name: "plan", text: "v1" });

    expect([created.owner, created.text, announced]).toEqual([USER, "v1", [{ name: "pad.changed", data: { name: "plan" } }]]);
  });

  it("u26_creating_a_pad_that_exists_fails_with_conflict", async () => {
    const { app } = pads([pad("plan")]);

    await expect(app.rpc("pad.create", { name: "plan", text: null })).rejects.toMatchObject({ code: -32003 });
  });

  it("u26_creating_a_pad_with_a_blank_name_fails_with_invalid_params", async () => {
    const { app } = pads([]);

    await expect(app.rpc("pad.create", { name: " ", text: null })).rejects.toMatchObject({ code: -32602 });
  });

  it("u26_writing_a_pad_replaces_its_text_and_sends_pad_changed", async () => {
    const { app, announced } = pads([pad("plan", "old")]);

    const written = await app.rpc("pad.write", { name: "plan", text: "new" });

    expect([written.text, announced]).toEqual(["new", [{ name: "pad.changed", data: { name: "plan" } }]]);
  });

  it("u26_appending_to_a_pad_adds_to_the_end_of_its_text", async () => {
    const { app } = pads([pad("plan", "old")]);

    const appended = await app.rpc("pad.append", { name: "plan", text: " more" });

    expect(appended.text).toBe("old more");
  });

  it("u26_setting_the_owner_of_a_pad_changes_who_owns_it", async () => {
    const { app } = pads([pad("plan")]);
    const agent = { kind: "agent", id: "agent-1", parent: null } as const;

    const owned = await app.rpc("pad.setOwner", { name: "plan", owner: agent });

    expect(owned.owner).toEqual(agent);
  });

  it("u26_a_pad_that_does_not_exist_fails_with_not_found", async () => {
    const { app } = pads([]);

    await expect(app.rpc("pad.read", { name: "nope" })).rejects.toMatchObject({ code: -32001 });
  });
});
