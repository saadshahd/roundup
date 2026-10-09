import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { AGENT, openPad, openShelf, padOf, RpcError } from "./padsFixture";

afterEach(cleanup);

/** jsdom lays nothing out, so the text field's scroll geometry is set by hand: a body 1510 px tall in a field the reader has scrolled to 1082. */
const scrolledReader = (field: HTMLTextAreaElement) => {
  Object.defineProperty(field, "scrollHeight", { configurable: true, get: () => 1510 });
  field.scrollTop = 1082;
};

const appendLine = async (text: string) => {
  const append = await screen.findByLabelText<HTMLInputElement>("append");

  fireEvent.input(append, { target: { value: text } });
  fireEvent.keyDown(append, { key: "Enter" });
};

describe("u141 the user's own append shows the new line", () => {
  it("u141_after_the_users_append_the_text_scrolls_to_its_end", async () => {
    await openShelf([padOf("auth-notes", AGENT, "a")]);
    const field = await openPad("auth-notes");
    scrolledReader(field);

    await appendLine(" more");
    await waitFor(() => expect(field.textContent).toContain("a more"));

    await waitFor(() => expect(field.scrollTop).toBe(1510));
  });

  it("u141_a_pad_changed_from_someone_else_keeps_the_reader_where_they_are", async () => {
    const { app, state } = await openShelf([padOf("auth-notes", AGENT, "v1")]);
    const field = await openPad("auth-notes");
    scrolledReader(field);
    state.pads = [padOf("auth-notes", AGENT, "v2")];

    app.emit({ actor: AGENT, name: "pad.changed", data: { name: "auth-notes" } });
    await waitFor(() => expect(field.textContent).toContain("v2"));

    expect(field.scrollTop).toBe(1082);
  });

  it("u141_a_failed_append_keeps_the_reader_where_they_are", async () => {
    const { app } = await openShelf([padOf("auth-notes", AGENT, "a")]);
    const field = await openPad("auth-notes");
    scrolledReader(field);
    app.handlers["pad.append"] = () => Promise.reject(new RpcError(-32000, "no room"));

    await appendLine(" more");
    await screen.findByText(/no room/);

    expect(field.scrollTop).toBe(1082);
  });
});
