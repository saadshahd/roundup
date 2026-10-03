import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import {
  AGENT,
  deferred,
  openPad,
  openShelf,
  padOf,
  waitForReload,
  writeAdopted,
} from "./padsFixture";
import { USER } from "../testing/nodes";
import type { Pad } from "@contracts/pad/Pad";

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

  it("u36_typing_more_while_the_line_shows_keeps_the_line_and_blur_sends_no_write", async () => {
    const { calls, field, state } = await openConflict();

    fireEvent.input(field, { target: { value: "mine, edited more" } });
    fireEvent.blur(field);

    expect(screen.getByText("changed by auth-refactor")).toBeTruthy();
    expect(calls()).not.toContain("pad.write");
    expect(field.value).toBe("mine, edited more");
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

  it("u36_keep_mine_keeps_the_line_while_its_write_is_in_flight", async () => {
    const { app } = await openConflict();
    const reply = deferred<Pad>();
    app.handlers["pad.write"] = () => reply.promise;

    fireEvent.click(screen.getByText("keep mine"));

    expect(screen.getByText("changed by auth-refactor")).toBeTruthy();

    reply.resolve(padOf("release-checklist", USER, "mine, edited"));
    await waitFor(() => expect(screen.queryByText(/changed by/)).toBeNull());
  });

  it("u36_keep_mine_never_overwrites_text_typed_after_a_quick_refocus", async () => {
    const { app, field } = await openConflict();
    const reply = deferred<Pad>();
    app.handlers["pad.write"] = () => reply.promise;

    fireEvent.click(screen.getByText("keep mine"));
    await waitFor(() =>
      expect(app.calls.some((call) => call.method === "pad.write")).toBe(true),
    );

    fireEvent.focus(field);
    fireEvent.input(field, { target: { value: "mine, edited more" } });
    reply.resolve(padOf("release-checklist", USER, "mine, edited"));
    await reply.promise;
    await new Promise((done) => setTimeout(done, 0));

    expect(field.value).toBe("mine, edited more");
  });

  it("u36_a_later_agent_change_refreshes_the_field_after_keep_mine_with_no_blur", async () => {
    const { app, field, state } = await openConflict();

    fireEvent.click(screen.getByText("keep mine"));
    await waitFor(() =>
      expect(app.calls.some((call) => call.method === "pad.write")).toBe(true),
    );

    state.pads = [padOf("release-checklist", USER, "mine, edited\nagent again")];
    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "release-checklist" },
    });

    await waitFor(() =>
      expect(field.value).toBe("mine, edited\nagent again"),
    );
    expect(screen.queryByText(/changed by/)).toBeNull();
  });

  it("u36_leaving_the_field_after_keep_mine_with_no_blur_sends_no_second_write", async () => {
    const { app, field } = await openConflict();

    fireEvent.click(screen.getByText("keep mine"));
    await waitFor(() =>
      expect(
        app.calls.filter((call) => call.method === "pad.write").length,
      ).toBe(1),
    );

    fireEvent.blur(field);
    await new Promise((done) => setTimeout(done, 0));

    expect(
      app.calls.filter((call) => call.method === "pad.write"),
    ).toHaveLength(1);
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

    await waitFor(() => expect(field.value).toBe("mine plus agent"));
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

  it("u36_a_reload_with_no_tracked_actor_does_not_clear_a_showing_conflict_line", async () => {
    const { app, state } = await openConflict();

    state.pads = [padOf("release-checklist", USER, "mine\nagent line\nmore")];
    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "other-pad" },
    });
    await waitForReload(app, 2);

    expect(screen.getByText("changed by auth-refactor")).toBeTruthy();
  });

  it("u36_a_save_that_settles_while_editing_moves_the_starting_text_forward_without_disturbing_the_draft", async () => {
    const { app } = await openShelf([
      padOf("release-checklist", USER, "old"),
    ]);

    const reply = deferred<Pad>();
    app.handlers["pad.write"] = () => reply.promise;

    const field = await openPad("release-checklist");
    fireEvent.focus(field);
    fireEvent.input(field, { target: { value: "new" } });
    fireEvent.blur(field);
    fireEvent.focus(field);
    fireEvent.input(field, { target: { value: "newer" } });
    reply.resolve(padOf("release-checklist", USER, "new"));
    await reply.promise;
    await writeAdopted();

    fireEvent.input(field, { target: { value: "old" } });

    expect(field.value).toBe("old");
    expect(screen.queryByText(/changed by/)).toBeNull();
  });

  it("u36_a_save_that_settles_while_editing_lets_typing_back_to_the_saved_text_refresh_on_another_actors_change", async () => {
    const { app, state } = await openShelf([
      padOf("release-checklist", USER, "old"),
    ]);

    const reply = deferred<Pad>();
    app.handlers["pad.write"] = () => reply.promise;

    const field = await openPad("release-checklist");
    fireEvent.focus(field);
    fireEvent.input(field, { target: { value: "new" } });
    fireEvent.blur(field);
    fireEvent.focus(field);
    fireEvent.input(field, { target: { value: "newer" } });
    reply.resolve(padOf("release-checklist", USER, "new"));
    await reply.promise;
    await writeAdopted();

    fireEvent.input(field, { target: { value: "new" } });
    state.pads = [padOf("release-checklist", USER, "new plus agent")];
    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "release-checklist" },
    });

    await waitFor(() => expect(field.value).toBe("new plus agent"));
    expect(screen.queryByText(/changed by/)).toBeNull();
  });

  it("u36_use_theirs_redraws_the_field_when_the_latest_text_matches_a_stale_draft_baseline", async () => {
    const { app, state } = await openShelf([
      padOf("release-checklist", USER, "mine"),
    ]);

    const field = await openPad("release-checklist");
    fireEvent.focus(field);
    fireEvent.input(field, { target: { value: "mine, edited" } });
    state.pads = [padOf("release-checklist", USER, "x")];
    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "release-checklist" },
    });
    await waitForReload(app);
    state.pads = [padOf("release-checklist", USER, "mine")];
    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "release-checklist" },
    });
    await waitForReload(app, 2);

    fireEvent.click(screen.getByText("use theirs"));

    expect(field.value).toBe("mine");
    expect(screen.queryByText(/changed by/)).toBeNull();

    fireEvent.blur(field);

    expect(app.calls.some((call) => call.method === "pad.write")).toBe(false);
    expect(state.pads[0]?.text).toBe("mine");
  });

  it("u36_a_change_that_lands_while_a_save_from_an_earlier_edit_is_still_in_flight_redraws_the_field_even_when_it_matches_the_stale_draft", async () => {
    const { app, state } = await openShelf([
      padOf("release-checklist", USER, "mine"),
    ]);

    const reply = deferred<Pad>();
    app.handlers["pad.write"] = () => reply.promise;

    const field = await openPad("release-checklist");
    fireEvent.focus(field);
    fireEvent.input(field, { target: { value: "mine, edited" } });
    fireEvent.blur(field);
    fireEvent.focus(field);
    fireEvent.input(field, { target: { value: "mine, edited" } });
    reply.resolve(padOf("release-checklist", USER, "mine, edited"));
    await reply.promise;
    await writeAdopted();

    state.pads = [padOf("release-checklist", USER, "mine")];
    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "release-checklist" },
    });
    await waitForReload(app);

    expect(field.value).toBe("mine");

    fireEvent.blur(field);

    expect(app.calls.filter((call) => call.method === "pad.write")).toHaveLength(1);
    expect(state.pads[0]?.text).toBe("mine");
  });

  it("u36_an_unrelated_event_sharing_the_pads_name_is_never_mistaken_for_a_pad_changed", async () => {
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
      data: { name: "release-checklist" },
    });
    // SAFETY: no real Event member carries `data.name`, except pad.changed itself, so this
    // shape is built the way a boundary value would arrive (round-tripped through JSON, as
    // the Daemon's own notifications do) rather than asserted straight from an object literal.
    // Fired in the same tick, before pad.list's reply resolves and the effect reads
    // pendingActor: it proves the subscribe filter at PadDrawer.tsx:58 keys on event.name,
    // not just the coincidence that data.name matches the Pad's name.

    // JSON.parse returns `any`, so `impostor` carries no asserted type here; the contract's
    // `rail.changed` variant has no `data`, and a cast would force a shape the type checker
    // has already proven wrong.
    const impostor = JSON.parse(
      JSON.stringify({
        actor: USER,
        name: "rail.changed",
        data: { name: "release-checklist" },
      }),
    );

    app.emit(impostor);
    await waitForReload(app);

    expect(screen.getByText("changed by auth-refactor")).toBeTruthy();
    expect(field.value).toBe("mine, edited");
  });
});
