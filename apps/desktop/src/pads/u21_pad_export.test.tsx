import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { AGENT, openShelf, padOf, RpcError } from "./padsFixture";

afterEach(cleanup);

const clickExport = async () => {
  fireEvent.click(await screen.findByText("auth-notes"));
  fireEvent.click(await screen.findByText("export .md"));
};

describe("u21 export", () => {
  it("u21_export_opens_the_save_chooser_with_the_pad_name_dot_md_filled_in", async () => {
    const { app } = await openShelf([padOf("auth-notes", AGENT)]);

    await clickExport();
    await waitFor(() => expect(app.chooser.suggestedNames).toHaveLength(1));

    expect(app.chooser.suggestedNames).toEqual(["auth-notes.md"]);
  });

  it("u21_choosing_a_path_calls_pad_export_with_it", async () => {
    const { app } = await openShelf([padOf("auth-notes", AGENT)]);
    app.chooser.savePath = "/Users/me/auth-notes.md";
    app.handlers["pad.export"] = () => null;

    await clickExport();
    await waitFor(() =>
      expect(app.calls.some((call) => call.method === "pad.export")).toBe(true),
    );

    expect(app.calls.filter((call) => call.method === "pad.export")).toEqual([
      {
        method: "pad.export",
        params: { name: "auth-notes", path: "/Users/me/auth-notes.md" },
      },
    ]);
  });

  it("u21_cancelling_the_chooser_calls_nothing", async () => {
    const { app, calls } = await openShelf([padOf("auth-notes", AGENT)]);
    app.chooser.savePath = null;

    await clickExport();
    await waitFor(() => expect(app.chooser.suggestedNames).toHaveLength(1));

    expect([
      app.chooser.suggestedNames,
      calls().includes("pad.export"),
    ]).toEqual([["auth-notes.md"], false]);
  });

  it("u21_an_export_error_shows_the_message_in_the_drawer", async () => {
    const { app } = await openShelf([padOf("auth-notes", AGENT)]);
    app.chooser.savePath = "/gone/auth-notes.md";
    app.handlers["pad.export"] = () => {
      throw new RpcError(-32602, "directory /gone does not exist");
    };

    await clickExport();

    const line = await screen.findByText(/does not exist/);

    expect([
      line.textContent,
      screen.getByLabelText("drawer").contains(line),
    ]).toEqual(["✕ directory /gone does not exist", true]);
  });
});
