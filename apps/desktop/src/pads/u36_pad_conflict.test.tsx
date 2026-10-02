import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { AGENT, openPad, openShelf, padOf, waitForReload } from "./padsFixture";
import { USER } from "../testing/nodes";

afterEach(cleanup);

/** The user owns `release-checklist`, edits it to a dirty draft, then another Actor's `pad.changed` arrives. */
const openConflict = async () => {
  const { app, state, calls } = await openShelf([
    padOf("release-checklist", USER, "mine"),
  ]);

  const field = await openPad("release-checklist");
  fireEvent.focus(field);
  fireEvent.input(field, { target: { value: "mine, edited" } });
  state.pads = [padOf("release-checklist", USER, "mine\nagent line")];
  app.emit({
    actor: AGENT,
    name: "pad.changed",
    data: { name: "release-checklist" },
  });
  await waitForReload(app);

  return { app, state, calls, field };
};

describe("u36 a pad edit never overwrites another actor's change", () => {
  it("u36_a_dirty_draft_shows_a_changed_by_line", async () => {
    await openConflict();

    expect(screen.getByText("changed by auth-refactor")).toBeTruthy();
  });

  it("u36_the_changed_by_line_is_light", async () => {
    await openConflict();

    expect(screen.getByText("changed by auth-refactor").className).toContain(
      "light",
    );
  });

  it("u36_a_dirty_draft_blur_sends_no_write", async () => {
    const { calls, field, state } = await openConflict();

    fireEvent.blur(field);

    expect(calls()).not.toContain("pad.write");
    expect(field.value).toBe("mine, edited");
    expect(state.pads[0]?.text).toBe("mine\nagent line");
  });

  it("u36_a_second_agents_change_after_blur_leaves_an_unresolved_conflicts_draft_untouched", async () => {
    const { app, field, state } = await openConflict();

    fireEvent.blur(field);

    state.pads = [padOf("release-checklist", USER, "mine\nagent line\nmore")];
    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "release-checklist" },
    });
    await waitForReload(app, 2);

    expect(field.value).toBe("mine, edited");
    expect(screen.getByText("changed by auth-refactor")).toBeTruthy();
  });

  it("u36_showing_the_line_reads_nothing_from_the_daemon", async () => {
    const { calls } = await openConflict();

    expect(calls().filter((method) => method === "pad.read")).toHaveLength(1);
  });

  it("u36_keep_mine_writes_the_draft_and_clears_the_line", async () => {
    const { app, field, state } = await openConflict();

    fireEvent.blur(field);
    fireEvent.click(screen.getByText("keep mine"));

    await waitFor(() =>
      expect(
        app.calls.filter((call) => call.method === "pad.write"),
      ).toEqual([
        { method: "pad.write", params: { name: "release-checklist", text: "mine, edited" } },
      ]),
    );
    expect(state.pads[0]?.text).toBe("mine, edited");
    expect(screen.queryByText(/changed by/)).toBeNull();
  });

  it("u36_keep_mine_clears_the_line_without_a_blur_first", async () => {
    const { app } = await openConflict();

    fireEvent.click(screen.getByText("keep mine"));

    await waitFor(() =>
      expect(app.calls.some((call) => call.method === "pad.write")).toBe(true),
    );
    expect(screen.queryByText(/changed by/)).toBeNull();
  });

  it("u36_use_theirs_loads_the_latest_text_and_calls_nothing_else", async () => {
    const { app, field, state } = await openConflict();

    fireEvent.blur(field);
    const before = app.calls.length;
    fireEvent.click(screen.getByText("use theirs"));

    expect(field.value).toBe("mine\nagent line");
    expect(app.calls.length).toBe(before);
    expect(state.pads[0]?.text).toBe("mine\nagent line");
    expect(screen.queryByText(/changed by/)).toBeNull();
  });

  it("u36_typing_back_to_the_original_text_clears_the_changed_by_line", async () => {
    const { field } = await openConflict();

    expect(screen.getByText("changed by auth-refactor")).toBeTruthy();

    fireEvent.input(field, { target: { value: "mine" } });

    expect(screen.queryByText(/changed by/)).toBeNull();
    expect(field.value).toBe("mine\nagent line");
  });

  it("u36_a_clean_draft_refreshes_immediately_even_while_focused", async () => {
    const { app, state } = await openShelf([
      padOf("release-checklist", USER, "mine"),
    ]);

    const field = await openPad("release-checklist");
    fireEvent.focus(field);
    fireEvent.input(field, { target: { value: "mine" } });
    state.pads = [padOf("release-checklist", USER, "mine plus agent")];

    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "release-checklist" },
    });

    expect(await screen.findByDisplayValue("mine plus agent")).toBe(field);
    expect(screen.queryByText(/changed by/)).toBeNull();
  });

  it("u36_the_users_own_write_echo_never_shows_the_line", async () => {
    const { app, state } = await openShelf([
      padOf("release-checklist", USER, "mine"),
    ]);

    const field = await openPad("release-checklist");
    fireEvent.focus(field);
    fireEvent.input(field, { target: { value: "mine, typing" } });
    state.pads = [padOf("release-checklist", USER, "mine plus agent")];

    app.emit({
      actor: USER,
      name: "pad.changed",
      data: { name: "release-checklist" },
    });
    await waitForReload(app);

    expect(screen.queryByText(/changed by/)).toBeNull();
    expect(field.value).toBe("mine, typing");
  });

  it("u36_a_pad_changed_for_another_pad_never_attributes_a_conflict_actor", async () => {
    const { app, state } = await openShelf([
      padOf("release-checklist", USER, "mine"),
    ]);

    const field = await openPad("release-checklist");
    fireEvent.focus(field);
    fireEvent.input(field, { target: { value: "mine, edited" } });
    state.pads = [padOf("release-checklist", USER, "mine\nagent line")];

    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "other-pad" },
    });
    await waitForReload(app);

    expect(screen.queryByText(/changed by/)).toBeNull();
    expect(field.value).toBe("mine, edited");
  });

  it("u36_a_conflict_line_clears_when_the_pad_is_deleted", async () => {
    const { app, state } = await openConflict();

    state.pads = [];
    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "release-checklist" },
    });

    await screen.findByText(/was deleted/);
    expect(screen.queryByText(/changed by/)).toBeNull();
  });

  it("u36_a_pad_changed_with_no_content_change_does_not_show_the_line", async () => {
    const { app } = await openShelf([
      padOf("release-checklist", USER, "mine"),
    ]);

    const field = await openPad("release-checklist");
    fireEvent.focus(field);
    fireEvent.input(field, { target: { value: "mine, edited" } });

    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "release-checklist" },
    });
    await waitForReload(app);

    expect(screen.queryByText(/changed by/)).toBeNull();
    expect(field.value).toBe("mine, edited");
  });

  it("u36_a_stale_conflict_actor_is_not_reused_for_a_later_unrelated_reload", async () => {
    const { app, state } = await openShelf([
      padOf("release-checklist", USER, "mine"),
    ]);

    const field = await openPad("release-checklist");
    fireEvent.focus(field);
    fireEvent.input(field, { target: { value: "mine, edited" } });

    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "release-checklist" },
    });
    await waitForReload(app);
    expect(screen.queryByText(/changed by/)).toBeNull();

    state.pads = [padOf("release-checklist", USER, "mine\nagent line")];
    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "other-pad" },
    });
    await waitForReload(app, 2);

    expect(screen.queryByText(/changed by/)).toBeNull();
    expect(field.value).toBe("mine, edited");
  });
});
