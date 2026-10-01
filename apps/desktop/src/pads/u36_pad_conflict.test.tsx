import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { AGENT, openShelf, padOf } from "./padsFixture";
import { USER } from "../testing/nodes";

afterEach(cleanup);

const openPad = async (name: string) => {
  fireEvent.click(await screen.findByText(name));

  return screen.findByLabelText<HTMLTextAreaElement>("text");
};

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
  await waitFor(() =>
    expect(
      calls().filter((method) => method === "pad.list").length,
    ).toBeGreaterThan(1),
  );

  return { app, state, calls, field };
};

describe("u36 a pad edit never overwrites another actor's change", () => {
  it("u36_a_dirty_draft_shows_a_changed_by_line_and_blur_sends_no_write", async () => {
    const { calls, field, state } = await openConflict();

    expect(screen.getByText("changed by auth-refactor")).toBeTruthy();

    fireEvent.blur(field);

    expect(calls()).not.toContain("pad.write");
    expect(field.value).toBe("mine, edited");
    expect(state.pads[0]?.text).toBe("mine\nagent line");
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

  it("u36_use_theirs_loads_the_latest_text_with_no_write", async () => {
    const { app, field, state } = await openConflict();

    fireEvent.blur(field);
    fireEvent.click(screen.getByText("use theirs"));

    expect(field.value).toBe("mine\nagent line");
    expect(app.calls.some((call) => call.method === "pad.write")).toBe(false);
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
    const { app, state, calls } = await openShelf([
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
    await waitFor(() =>
      expect(
        calls().filter((method) => method === "pad.list").length,
      ).toBeGreaterThan(1),
    );

    expect(screen.queryByText(/changed by/)).toBeNull();
    expect(field.value).toBe("mine, typing");
  });
});
