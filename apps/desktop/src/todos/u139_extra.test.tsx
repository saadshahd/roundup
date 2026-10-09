import { cleanup, fireEvent, screen, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { mountTodos, todo } from "./testHarness";

afterEach(cleanup);

const drawer = () => screen.getByRole("complementary", { name: "drawer" });

describe("u139 a non-Enter key on the focused text does not start editing", () => {
  it("u139_a_non_enter_key_on_the_title_does_not_start_editing", async () => {
    await mountTodos([todo(5, { title: "session store", body: "Move the cache." })]);
    fireEvent.click(await screen.findByRole("button", { name: /#5 / }));
    await within(drawer()).findByText("blocker");
    const title = within(drawer()).getByText("session store");

    title.focus();
    fireEvent.keyDown(title, { key: "a" });

    expect(within(drawer()).queryByLabelText("title")).toBeNull();
  });
});
