import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import type { Pad } from "@contracts/pad/Pad";
import { afterEach, describe, expect, it } from "vitest";
import {
  AGENT,
  deferred,
  openShelf,
  padOf,
  RpcError,
} from "./padsFixture";
import { USER } from "../testing/nodes";

afterEach(cleanup);

describe("u18 list and ownership", () => {
  it("u18_the_pads_list_shows_each_pad_by_name", async () => {
    await openShelf([
      padOf("auth-notes", AGENT),
      padOf("release-checklist", USER),
    ]);

    expect(
      [
        await screen.findByText("auth-notes"),
        screen.getByText("release-checklist"),
      ].length,
    ).toBe(2);
  });

  it("u18_a_pad_name_is_plain_text_not_a_light_word", async () => {
    await openShelf([padOf("auth-notes", AGENT)]);

    expect((await screen.findByText("auth-notes")).className).not.toContain(
      "word",
    );
  });

  it("u18_an_agents_pad_carries_the_mark_◈_and_the_users_pad_the_mark_◇", async () => {
    await openShelf([
      padOf("auth-notes", AGENT),
      padOf("release-checklist", USER),
    ]);
    await screen.findByText("auth-notes");

    expect([
      screen.getAllByText("◈").length,
      screen.getAllByText("◇").length,
    ]).toEqual([1, 1]);
  });

  it("u18_clicking_◈_calls_pad_setOwner_with_the_user_and_the_mark_becomes_◇", async () => {
    const { app, state } = await openShelf([padOf("auth-notes", AGENT)]);
    app.handlers["pad.setOwner"] = ({ name, owner }) => {
      state.pads = state.pads.map((pad) =>
        pad.name === name ? { ...pad, owner } : pad,
      );

      return padOf(name, owner);
    };

    fireEvent.click(await screen.findByText("◈"));
    await screen.findByText("◇");

    expect(app.calls.filter((call) => call.method === "pad.setOwner")).toEqual([
      { method: "pad.setOwner", params: { name: "auth-notes", owner: USER } },
    ]);
    expect(screen.queryByText("◈")).toBeNull();
  });

  it("u18_the_mark_◇_is_not_clickable", async () => {
    const { calls } = await openShelf([padOf("release-checklist", USER)]);

    fireEvent.click(await screen.findByText("◇"));

    expect(calls()).not.toContain("pad.setOwner");
  });

  it("u18_a_refused_owner_change_shows_the_message_in_ink", async () => {
    const { app } = await openShelf([padOf("auth-notes", AGENT)]);
    app.handlers["pad.setOwner"] = () => {
      throw new RpcError(-32001, "only the owner may hand it over");
    };

    fireEvent.click(await screen.findByText("◈"));

    expect((await screen.findByText(/only the owner/)).textContent).toBe(
      "✕ only the owner may hand it over",
    );
  });

  it("u18_pad_changed_refetches_pad_list_and_logs_no_touch", async () => {
    const { app, state, calls } = await openShelf([padOf("auth-notes", AGENT)]);
    await screen.findByText("auth-notes");
    state.pads = [...state.pads, padOf("migrate-plan", AGENT)];

    app.emit({
      actor: AGENT,
      name: "pad.changed",
      data: { name: "migrate-plan" },
    });
    await screen.findByText("migrate-plan");

    expect(
      calls().filter(
        (method) => method !== "rail.tree" && method !== "terminal.list",
      ),
    ).toEqual(["pad.list", "pad.list"]);
    await waitFor(() => expect(calls()).not.toContain("pad.read"));
  });

  it("u18_an_older_pad_list_reply_that_arrives_late_never_wins", async () => {
    const { app } = await openShelf([padOf("auth-notes", AGENT)]);
    await screen.findByText("auth-notes");
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

    replies.at(-1)?.resolve([padOf("newest", AGENT)]);
    await screen.findByText("newest");
    replies
      .slice(0, newest)
      .forEach((older) => older.resolve([padOf("older", AGENT)]));
    await Promise.resolve();
    await Promise.resolve();

    expect(screen.queryByText("older")).toBeNull();
  });
});
