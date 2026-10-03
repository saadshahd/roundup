import { cleanup, fireEvent, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { mountTodos, todo } from "./testHarness";

afterEach(cleanup);

const drawer = () => screen.getByRole("complementary", { name: "drawer" });

const openDrawer = async () => {
  await mountTodos([todo(5, { title: "session store", body: "Move the cache." })]);
  fireEvent.click(await screen.findByRole("button", { name: /#5 / }));
  await within(drawer()).findByText("+ blocker");
};

describe("u139 a Todo's title and body edit by keyboard", () => {
  it("u139_the_title_and_the_body_are_tab_stops", async () => {
    await openDrawer();

    expect([within(drawer()).getByText("session store").tabIndex, within(drawer()).getByText("Move the cache.").tabIndex]).toEqual([0, 0]);
  });

  it("u139_enter_on_the_title_opens_its_field_with_focus_in_it", async () => {
    await openDrawer();
    const title = within(drawer()).getByText("session store");

    title.focus();
    fireEvent.keyDown(title, { key: "Enter" });

    await waitFor(() => expect(document.activeElement).toBe(within(drawer()).getByLabelText("title")));
  });

  it("u139_enter_on_the_body_opens_its_field_with_focus_in_it", async () => {
    await openDrawer();
    const body = within(drawer()).getByText("Move the cache.");

    body.focus();
    fireEvent.keyDown(body, { key: "Enter" });

    await waitFor(() => expect(document.activeElement).toBe(within(drawer()).getByLabelText("body")));
  });
});
