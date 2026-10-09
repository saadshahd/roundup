import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { AGENT, openPad, openShelf, padOf, RpcError } from "./padsFixture";
import { USER } from "../testing/nodes";

afterEach(cleanup);

const calls = (app: Awaited<ReturnType<typeof openShelf>>["app"]) => app.calls.filter((call) => call.method === "pad.delete");

const ask = () => fireEvent.click(screen.getByRole("button", { name: "delete" }));

describe("u150 delete asks first (Pad)", () => {
  it("u150_a_user_pad_asks_in_place_and_calls_nothing", async () => {
    const { app } = await openShelf([padOf("notes", USER, "a")]);
    await openPad("notes");

    ask();

    expect([screen.getByText(/delete notes\?/).textContent, calls(app)]).toEqual(["delete notes?  delete keep", []]);
  });

  it("u150_an_agent_pad_names_its_owner", async () => {
    await openShelf([padOf("notes", AGENT, "a")]);
    await openPad("notes");

    ask();

    expect(screen.getByText(/delete notes\?/).textContent).toContain("auth-refactor owns it; you may delete any Pad");
  });

  it("u150_a_dirty_draft_adds_the_unsaved_warning", async () => {
    await openShelf([padOf("notes", USER, "a")]);
    const field = await openPad("notes");
    fireEvent.input(field, { target: { value: "changed" } });

    ask();

    expect(screen.getByText(/delete notes\?/).textContent).toContain("unsaved changes are lost");
  });

  it("u150_confirming_calls_pad_delete_once_closes_with_no_deleted_line", async () => {
    const { app, state, connected } = await openShelf([padOf("notes", USER, "a")]);
    app.handlers["pad.delete"] = () => {
      state.pads = [];

      return null;
    };

    await openPad("notes");
    ask();

    fireEvent.click(screen.getAllByRole("button", { name: "delete" })[0]!);
    app.emit({ actor: USER, name: "pad.changed", data: { name: "notes" } });

    await waitFor(() => expect(connected.drawer.content()).toBeNull());
    expect([calls(app), screen.queryByText(/was deleted/)]).toEqual([[{ method: "pad.delete", params: { name: "notes" } }], null]);
    await waitFor(() => expect(screen.queryByText("notes")).toBeNull());
  });

  it("u150_keep_calls_nothing_and_focus_returns_to_delete", async () => {
    const { app } = await openShelf([padOf("notes", USER, "a")]);
    await openPad("notes");
    ask();
    const keep = screen.getByRole("button", { name: "keep" });
    await waitFor(() => expect(document.activeElement).toBe(keep));

    fireEvent.click(keep);

    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("button", { name: "delete" })));
    expect(calls(app)).toEqual([]);
  });

  it("u150_esc_closes_the_drawer_and_calls_nothing", async () => {
    const { app, connected } = await openShelf([padOf("notes", USER, "a")]);
    await openPad("notes");
    ask();

    fireEvent.keyDown(document.body, { key: "Escape" });

    expect([connected.drawer.content(), calls(app)]).toEqual([null, []]);
  });

  it("u150_an_error_keeps_the_drawer_and_the_pad", async () => {
    const { app, connected } = await openShelf([padOf("notes", USER, "a")]);
    app.handlers["pad.delete"] = () => Promise.reject(new RpcError(-32002, "forbidden"));
    await openPad("notes");
    ask();

    fireEvent.click(screen.getAllByRole("button", { name: "delete" })[0]!);

    expect((await screen.findByText(/forbidden/)).textContent).toBe("forbidden");
    expect(connected.drawer.content()).not.toBeNull();
    expect(screen.getAllByText("notes").length).toBeGreaterThan(0);
  });
});
