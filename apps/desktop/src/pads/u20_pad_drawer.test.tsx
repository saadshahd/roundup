import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import {
  AGENT,
  deferred,
  openShelf,
  padOf,
  RpcError,
} from "./padsFixture";
import { USER } from "../testing/nodes";
import type { Pad } from "@contracts/pad/Pad";
import styles from "../styles.css?inline";

afterEach(cleanup);

const openPad = async (name: string) => {
  fireEvent.click(await screen.findByText(name));

  return screen.findByLabelText<HTMLTextAreaElement>("text");
};

const edit = (field: HTMLTextAreaElement, value: string) => {
  fireEvent.focus(field);
  fireEvent.input(field, { target: { value } });
  fireEvent.blur(field);
};

const methodsAfterOpen = (calls: string[]) =>
  calls.filter(
    (method) =>
      method !== "rail.tree" &&
      method !== "terminal.list" &&
      method !== "pad.list",
  );

describe("u20 open and edit", () => {
  it("u20_the_drawer_shows_the_owner_mark_the_name_and_who_owns_the_pad", async () => {
    await openShelf([padOf("auth-notes", AGENT, "x")]);
    await openPad("auth-notes");

    expect(screen.getByLabelText("drawer").textContent).toContain(
      "◈ auth-notes owned by auth-refactor",
    );
  });

  it("u20_the_drawer_offers_a_light_export_word", async () => {
    await openShelf([padOf("auth-notes", AGENT)]);
    await openPad("auth-notes");

    expect(screen.getByText("export .md").className).toContain("light");
  });

  it("u20_the_text_is_markdown_source_in_the_monospace_face", async () => {
    await openShelf([
      padOf("auth-notes", AGENT, "## refresh rotation\n- rotate on every use"),
    ]);

    const field = await openPad("auth-notes");

    expect([field.value, field.style.fontFamily]).toEqual([
      "## refresh rotation\n- rotate on every use",
      "var(--mono)",
    ]);
  });

  it("u20_opening_calls_provenance_history_then_pad_read_once", async () => {
    const { calls } = await openShelf([padOf("auth-notes", AGENT)]);

    await openPad("auth-notes");

    expect(methodsAfterOpen(calls())).toEqual([
      "provenance.history",
      "pad.read",
    ]);
  });

  it("u20_the_drawer_asks_for_the_history_of_the_pad_item", async () => {
    const { app } = await openShelf([padOf("auth-notes", AGENT)]);

    await openPad("auth-notes");

    expect(
      app.calls.find((call) => call.method === "provenance.history")?.params,
    ).toEqual({ item: "pad:auth-notes" });
  });

  it("u20_the_drawer_ends_with_the_last_touch_line", async () => {
    const { state } = await openShelf([padOf("auth-notes", AGENT)]);
    state.history = [
      {
        actor: AGENT,
        verb: "wrote",
        item: "pad:auth-notes",
        at: new Date(2026, 0, 5, 14, 22).getTime(),
      },
    ];

    await openPad("auth-notes");

    expect((await screen.findByText(/^last/)).textContent).toBe(
      "last  auth-refactor wrote 14:22",
    );
  });

  it("u20_a_pad_the_user_owns_has_editable_text", async () => {
    await openShelf([padOf("release-checklist", USER, "x")]);

    const field = await openPad("release-checklist");

    expect([field.readOnly, screen.queryByLabelText("append")]).toEqual([
      false,
      null,
    ]);
  });

  it("u20_leaving_the_field_calls_pad_write_once_when_the_text_changed", async () => {
    const { app } = await openShelf([padOf("release-checklist", USER, "old")]);
    const field = await openPad("release-checklist");

    edit(field, "new");
    await screen.findByDisplayValue("new");

    expect(app.calls.filter((call) => call.method === "pad.write")).toEqual([
      {
        method: "pad.write",
        params: { name: "release-checklist", text: "new" },
      },
    ]);
  });

  it("u20_leaving_the_field_with_unchanged_text_calls_nothing", async () => {
    const { calls } = await openShelf([
      padOf("release-checklist", USER, "same"),
    ]);

    const field = await openPad("release-checklist");

    edit(field, "same");

    expect(calls()).not.toContain("pad.write");
  });

  it("u20_a_pad_an_agent_owns_is_read_only_with_an_append_field", async () => {
    await openShelf([padOf("auth-notes", AGENT, "x")]);

    const field = await openPad("auth-notes");

    expect(
      [field.readOnly, await screen.findByLabelText("append")].map(Boolean),
    ).toEqual([true, true]);
  });

  it("u20_enter_in_the_append_field_calls_pad_append_and_clears_it", async () => {
    const { app } = await openShelf([padOf("auth-notes", AGENT, "a")]);
    await openPad("auth-notes");
    const append = await screen.findByLabelText<HTMLInputElement>("append");

    fireEvent.input(append, { target: { value: " more" } });
    fireEvent.keyDown(append, { key: "Enter" });
    await screen.findByDisplayValue("a more");

    expect(app.calls.filter((call) => call.method === "pad.append")).toEqual([
      { method: "pad.append", params: { name: "auth-notes", text: " more" } },
    ]);
    await waitFor(() => expect(append.value).toBe(""));
  });

  it("u20_the_export_word_sits_in_a_row_below_the_pinned_close_word", async () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;
    await openShelf([padOf("auth-notes", AGENT)]);
    await openPad("auth-notes");
    const close = screen.getByText("close");
    const exportWord = screen.getByText("export .md");
    const row = exportWord.closest("p");

    if (!row) throw new Error("export .md has no row");


    const layout = [
      getComputedStyle(close).position,
      getComputedStyle(close).top,
      getComputedStyle(close).right,
      getComputedStyle(exportWord).position,
      getComputedStyle(row).display,
      row.previousElementSibling?.tagName,
      getComputedStyle(row).textAlign,
    ];

    sheet.remove();

    expect(layout).toEqual(["absolute", "8px", "16px", "static", "block", "P", "right"]);
  });

  it("u20_pad_changed_for_the_open_pad_calls_only_pad_list_and_refreshes_its_text", async () => {
    const { app, state, calls } = await openShelf([
      padOf("auth-notes", AGENT, "v1"),
    ]);

    await openPad("auth-notes");
    const before = calls().length;
    state.pads = [padOf("auth-notes", AGENT, "v2")];

    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "auth-notes" },
    });
    await screen.findByDisplayValue("v2");

    expect(calls().slice(before)).toEqual(["pad.list"]);
  });

  it("u20_focusing_an_agents_read_only_text_does_not_stop_it_refreshing", async () => {
    const { app, state } = await openShelf([padOf("auth-notes", AGENT, "v1")]);
    const field = await openPad("auth-notes");
    fireEvent.focus(field);
    state.pads = [padOf("auth-notes", AGENT, "v2")];

    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "auth-notes" },
    });

    expect(await screen.findByDisplayValue("v2")).toBe(field);
  });

  it("u20_pad_changed_while_the_user_is_editing_leaves_the_text_alone", async () => {
    const { app, state, calls } = await openShelf([
      padOf("release-checklist", USER, "mine"),
    ]);

    const field = await openPad("release-checklist");
    fireEvent.focus(field);
    fireEvent.input(field, { target: { value: "mine, typing" } });
    state.pads = [padOf("release-checklist", USER, "theirs")];
    const before = calls().length;

    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "release-checklist" },
    });
    await waitFor(() => expect(calls().length).toBeGreaterThan(before));

    expect(field.value).toBe("mine, typing");
  });

  it("u20_pad_changed_for_another_pad_does_not_touch_the_open_pad", async () => {
    const { app, calls } = await openShelf([padOf("auth-notes", AGENT, "v1")]);
    await openPad("auth-notes");
    const before = calls().length;

    app.emit({ actor: AGENT, name: "pad.changed", data: { name: "other" } });
    await waitFor(() => expect(calls().length).toBeGreaterThan(before));

    expect(calls().slice(before)).toEqual(["pad.list"]);
  });

  it("u20_a_write_reply_never_overwrites_text_typed_after_a_quick_refocus", async () => {
    const { app } = await openShelf([padOf("release-checklist", USER, "old")]);
    const reply = deferred<Pad>();
    const writes: string[] = [];
    app.handlers["pad.write"] = ({ text }) => {
      writes.push(text);

      return writes.length === 1 ? reply.promise : padOf("release-checklist", USER, text);
    };

    const field = await openPad("release-checklist");
    edit(field, "new");

    fireEvent.input(field, { target: { value: "newer" } });
    reply.resolve(padOf("release-checklist", USER, "new"));
    await reply.promise;
    await new Promise((done) => setTimeout(done, 0));
    expect(field.value).toBe("newer");
    fireEvent.blur(field);

    await waitFor(() => expect(writes).toEqual(["new", "newer"]));
  });

  it("u20_clicking_back_in_while_a_save_is_in_flight_never_loses_an_agents_append", async () => {
    const { app, state } = await openShelf([padOf("release-checklist", USER, "old")]);
    const reply = deferred<Pad>();
    const writes: string[] = [];
    app.handlers["pad.write"] = ({ text }) => {
      writes.push(text);

      return reply.promise;
    };

    const field = await openPad("release-checklist");
    edit(field, "new");
    fireEvent.focus(field);
    reply.resolve(padOf("release-checklist", USER, "new"));
    await reply.promise;
    await new Promise((done) => setTimeout(done, 0));
    state.pads = [padOf("release-checklist", USER, "new plus agent")];
    app.emit({ actor: AGENT, name: "pad.changed", data: { name: "release-checklist" } });
    await waitFor(() => expect(field.value).toBe("new plus agent"));

    fireEvent.blur(field);
    await new Promise((done) => setTimeout(done, 0));

    expect([writes, state.pads[0]?.text]).toEqual([["new"], "new plus agent"]);
  });

  it("u20_typing_back_to_the_original_text_never_sends_it_over_an_agents_append", async () => {
    const { app, state } = await openShelf([padOf("release-checklist", USER, "mine")]);
    const field = await openPad("release-checklist");
    fireEvent.input(field, { target: { value: "mine, typing" } });
    state.pads = [padOf("release-checklist", USER, "mine plus agent")];
    app.emit({ actor: AGENT, name: "pad.changed", data: { name: "release-checklist" } });
    await waitFor(() => expect(app.calls.filter((call) => call.method === "pad.list").length).toBeGreaterThan(1));

    fireEvent.input(field, { target: { value: "mine" } });
    fireEvent.blur(field);

    await waitFor(() => expect(field.value).toBe("mine plus agent"));
    expect([app.calls.some((call) => call.method === "pad.write"), state.pads[0]?.text]).toEqual([false, "mine plus agent"]);
  });

  it("u20_a_pad_handed_over_while_the_cursor_is_in_the_text_saves_what_is_typed", async () => {
    const { app, state } = await openShelf([padOf("auth-notes", AGENT, "v1")]);
    const field = await openPad("auth-notes");
    fireEvent.focus(field);
    state.pads = [padOf("auth-notes", USER, "v1")];
    app.emit({ actor: AGENT, name: "pad.changed", data: { name: "auth-notes" } });
    await waitFor(() => expect(field.readOnly).toBe(false));

    fireEvent.input(field, { target: { value: "typed" } });
    fireEvent.blur(field);

    await waitFor(() => expect(state.pads[0]?.text).toBe("typed"));
    expect(app.calls.filter((call) => call.method === "pad.write")).toEqual([
      { method: "pad.write", params: { name: "auth-notes", text: "typed" } },
    ]);
  });

  it("u20_a_change_skipped_while_editing_refreshes_on_blur_when_nothing_was_typed", async () => {
    const { app, state } = await openShelf([
      padOf("release-checklist", USER, "mine"),
    ]);

    const field = await openPad("release-checklist");
    fireEvent.focus(field);
    state.pads = [padOf("release-checklist", USER, "mine plus")];
    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "release-checklist" },
    });
    await waitFor(() =>
      expect(
        app.calls.filter((call) => call.method === "pad.list").length,
      ).toBeGreaterThan(1),
    );

    fireEvent.blur(field);

    expect(await screen.findByDisplayValue("mine plus")).toBe(field);
    expect(app.calls.some((call) => call.method === "pad.write")).toBe(false);
  });

  it("u20_clicking_in_and_out_without_typing_never_sends_the_text_the_user_was_shown", async () => {
    const { app, state } = await openShelf([
      padOf("release-checklist", USER, "mine"),
    ]);

    const field = await openPad("release-checklist");
    fireEvent.focus(field);
    state.pads = [padOf("release-checklist", USER, "mine plus agent")];
    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "release-checklist" },
    });
    await waitFor(() =>
      expect(
        app.calls.filter((call) => call.method === "pad.list").length,
      ).toBeGreaterThan(1),
    );

    fireEvent.blur(field);

    expect(await screen.findByDisplayValue("mine plus agent")).toBe(field);
    expect(app.calls.some((call) => call.method === "pad.write")).toBe(false);
  });

  it("u20_a_background_refresh_does_not_clear_a_visible_error", async () => {
    const { app, calls } = await openShelf([padOf("auth-notes", AGENT, "v1")]);
    app.chooser.savePath = "/gone/auth-notes.md";
    app.handlers["pad.export"] = () => {
      throw new RpcError(-32602, "directory /gone does not exist");
    };

    await openPad("auth-notes");
    fireEvent.click(screen.getByText("export .md"));
    await screen.findByText(/does not exist/);
    const before = calls().length;

    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "auth-notes" },
    });
    await waitFor(() => expect(calls().length).toBeGreaterThan(before));

    expect(screen.queryByText(/does not exist/)).not.toBeNull();
  });

  it("u20_an_empty_append_calls_nothing", async () => {
    const { calls } = await openShelf([padOf("auth-notes", AGENT, "a")]);
    await openPad("auth-notes");
    const append = await screen.findByLabelText<HTMLInputElement>("append");

    fireEvent.keyDown(append, { key: "Enter" });

    expect(calls()).not.toContain("pad.append");
  });

  it("u20_an_older_pad_list_reply_that_arrives_late_never_wins", async () => {
    const { app } = await openShelf([padOf("auth-notes", AGENT, "v0")]);
    const field = await openPad("auth-notes");
    const replies: ReturnType<typeof deferred<Pad[]>>[] = [];
    app.handlers["pad.list"] = () => {
      const reply = deferred<Pad[]>();
      replies.push(reply);

      return reply.promise;
    };

    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "auth-notes" },
    });
    await waitFor(() => expect(replies.length).toBe(1));
    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "auth-notes" },
    });
    await waitFor(() => expect(replies.length).toBe(2));
    const newest = replies.length - 1;

    replies.at(-1)?.resolve([padOf("auth-notes", AGENT, "newest")]);
    await screen.findByDisplayValue("newest");
    replies
      .slice(0, newest)
      .forEach((older) => older.resolve([padOf("auth-notes", AGENT, "older")]));
    await Promise.resolve();
    await Promise.resolve();

    expect(field.value).toBe("newest");
  });

  it("u20_two_quick_enters_in_the_append_field_append_once", async () => {
    const { app } = await openShelf([padOf("auth-notes", AGENT, "a")]);
    await openPad("auth-notes");
    const append = await screen.findByLabelText<HTMLInputElement>("append");
    fireEvent.input(append, { target: { value: " more" } });

    fireEvent.keyDown(append, { key: "Enter" });
    fireEvent.keyDown(append, { key: "Enter" });
    await screen.findByDisplayValue("a more");

    expect(
      app.calls.filter((call) => call.method === "pad.append"),
    ).toHaveLength(1);
  });

  it("u20_a_failed_append_keeps_what_the_user_typed", async () => {
    const { app } = await openShelf([padOf("auth-notes", AGENT, "a")]);
    app.handlers["pad.append"] = () => {
      throw new RpcError(-32001, "pad is locked");
    };

    await openPad("auth-notes");
    const append = await screen.findByLabelText<HTMLInputElement>("append");
    fireEvent.input(append, { target: { value: " more" } });

    fireEvent.keyDown(append, { key: "Enter" });
    await screen.findByText(/pad is locked/);

    expect(append.value).toBe(" more");
  });

  it("u20_a_pad_deleted_while_its_drawer_is_open_says_so", async () => {
    const { app, state } = await openShelf([padOf("auth-notes", AGENT, "a")]);
    await openPad("auth-notes");
    state.pads = [];

    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "auth-notes" },
    });

    expect((await screen.findByText(/was deleted/)).textContent).toBe(
      "✕ pad auth-notes was deleted",
    );
  });

  it("u20_a_write_that_fails_after_the_drawer_closed_shows_its_message_on_the_shelf", async () => {
    const { app, connected } = await openShelf([
      padOf("release-checklist", USER, "old"),
    ]);

    const reply = deferred<Pad>();
    app.handlers["pad.write"] = () => reply.promise;
    const field = await openPad("release-checklist");
    edit(field, "new");
    await waitFor(() =>
      expect(app.calls.some((call) => call.method === "pad.write")).toBe(true),
    );

    connected.drawer.close();
    reply.reject(new RpcError(-32001, "only the owner may write"));

    expect(
      (await screen.findByText(/only the owner may write/)).textContent,
    ).toBe("✕ only the owner may write");
    expect(screen.queryByLabelText("drawer")?.textContent).not.toContain(
      "only the owner",
    );
  });
});
