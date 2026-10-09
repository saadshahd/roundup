import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { CONFLICT, openShelf, padOf, RpcError } from "./padsFixture";
import { USER } from "../testing/nodes";

afterEach(cleanup);

const typeName = async (name: string) => {
  fireEvent.click(await screen.findByRole("button", { name: "add pad" }));
  const field = await screen.findByLabelText<HTMLInputElement>("pad name");
  fireEvent.input(field, { target: { value: name } });
  fireEvent.keyDown(field, { key: "Enter" });

  return field;
};

describe("u19 create", () => {
  it("u19_clicking_plus_opens_an_inline_name_field", async () => {
    await openShelf([]);

    fireEvent.click(await screen.findByRole("button", { name: "add pad" }));

    expect(await screen.findByLabelText("pad name")).toBeTruthy();
  });

  it("u19_escape_closes_the_name_field_and_calls_nothing", async () => {
    const { calls } = await openShelf([]);
    fireEvent.click(await screen.findByRole("button", { name: "add pad" }));

    fireEvent.keyDown(await screen.findByLabelText("pad name"), {
      key: "Escape",
    });

    expect([
      screen.queryByLabelText("pad name"),
      calls().includes("pad.create"),
    ]).toEqual([null, false]);
  });

  it("u19_enter_calls_pad_create_with_the_name_and_closes_the_field", async () => {
    const { app } = await openShelf([]);
    app.handlers["pad.create"] = ({ name }) => padOf(name, USER);

    await typeName("notes");
    await screen.findByRole("button", { name: "add pad" });

    expect(app.calls.filter((call) => call.method === "pad.create")).toEqual([
      { method: "pad.create", params: { name: "notes", text: null } },
    ]);
    expect(screen.queryByLabelText("pad name")).toBeNull();
  });

  it("u19_a_conflict_shows_the_message_in_place_of_the_field", async () => {
    const { app } = await openShelf([]);
    app.handlers["pad.create"] = () => {
      throw new RpcError(CONFLICT, "pad notes exists");
    };

    await typeName("Notes");

    expect((await screen.findByText(/exists/)).textContent).toBe(
      "pad notes exists",
    );
    expect(screen.queryByLabelText("pad name")).toBeNull();
  });

  it("u19_invalid_params_shows_the_message_in_place_of_the_field", async () => {
    const { app } = await openShelf([]);
    app.handlers["pad.create"] = () => {
      throw new RpcError(-32602, "name must not contain /");
    };

    await typeName("a/b");

    expect((await screen.findByText(/must not contain/)).textContent).toBe(
      "name must not contain /",
    );
    expect(screen.queryByLabelText("pad name")).toBeNull();
  });

  it("u19_clicking_plus_again_replaces_the_message_with_a_fresh_field", async () => {
    const { app } = await openShelf([]);
    app.handlers["pad.create"] = () => {
      throw new RpcError(CONFLICT, "pad notes exists");
    };

    await typeName("notes");
    await screen.findByText(/exists/);

    fireEvent.click(screen.getByRole("button", { name: "add pad" }));

    expect(
      [
        await screen.findByLabelText("pad name"),
        screen.queryByText(/exists/),
      ].map(Boolean),
    ).toEqual([true, false]);
  });
});
