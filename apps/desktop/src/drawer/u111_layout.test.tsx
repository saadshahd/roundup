import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import styles from "../styles.css?inline";
import { openPad, openShelf, padOf, AGENT } from "../pads/padsFixture";
import { mountTodos, todo } from "../todos/testHarness";

afterEach(cleanup);

const withStylesheet = async <T,>(run: () => Promise<T>): Promise<T> => {
  const sheet = document.head.appendChild(document.createElement("style"));
  sheet.textContent = styles;

  try {
    return await run();
  } finally {
    sheet.remove();
  }
};

const focusCalls = () => vi.spyOn(HTMLElement.prototype, "focus");

const keptScroll = (spy: ReturnType<typeof focusCalls>) => {
  const options = spy.mock.calls.map(([option]) => option);
  spy.mockRestore();

  return options.length > 0 && options.every((option) => option?.preventScroll === true);
};

const drawerPanel = () => screen.getByLabelText("drawer");

describe("u111 opening and closing a Drawer leaves the layout where it was", () => {
  it("u111_the_element_holding_the_rail_the_terminal_and_the_shelf_clips_sideways", async () => {
    const overflow = await withStylesheet(async () => {
      await mountTodos([todo(1)]);
      const columns = getComputedStyle(document.querySelector(".columns")!);

      return [columns.overflowX, columns.overflowY];
    });

    expect(overflow).toEqual(["clip", "clip"]);
  });

  it("u111_every_focus_into_or_out_of_the_drawer_passes_prevent_scroll", async () => {
    const spy = focusCalls();
    const { connected } = await mountTodos([todo(1)]);

    connected.drawer.open(() => <input aria-label="field" />);
    await waitFor(() => expect(screen.getByLabelText("drawer").contains(document.activeElement)).toBe(true));
    connected.drawer.close();
    await waitFor(() => expect(screen.getByLabelText("drawer").inert).toBe(true));

    const options = spy.mock.calls.map(([option]) => option);
    spy.mockRestore();

    expect(options.length).toBeGreaterThan(0);
    expect(options.every((option) => option?.preventScroll === true)).toBe(true);
  });

  it("u111_closing_with_esc_also_passes_prevent_scroll", async () => {
    const outside = document.body.appendChild(document.createElement("button"));
    outside.focus();
    const spy = focusCalls();
    const { connected } = await mountTodos([todo(1)]);

    connected.drawer.open(() => <p>detail</p>);
    await waitFor(() => expect(screen.getByLabelText("drawer").contains(document.activeElement)).toBe(true));
    fireEvent.keyDown(document.body, { key: "Escape" });
    await waitFor(() => expect(screen.getByLabelText("drawer").inert).toBe(true));

    const options = spy.mock.calls.map(([option]) => option);
    spy.mockRestore();
    outside.remove();

    expect(options.length).toBeGreaterThan(0);
    expect(options.every((option) => option?.preventScroll === true)).toBe(true);
  });

  // Every Drawer opens through DrawerHost, and U64's close on a new selection calls the same `drawer.close()` (test above).
  it("u111_a_todo_drawer_opened_from_its_row_and_closed_with_close_passes_prevent_scroll", async () => {
    await mountTodos([todo(1)]);
    const spy = focusCalls();

    fireEvent.click(await screen.findByRole("button", { name: /todo 1/ }));
    await waitFor(() => expect(drawerPanel().contains(document.activeElement)).toBe(true));
    fireEvent.click(screen.getByRole("button", { name: "close" }));
    await waitFor(() => expect(drawerPanel().inert).toBe(true));

    expect(keptScroll(spy)).toBe(true);
  });

  it("u111_a_pad_drawer_opened_from_its_row_and_closed_with_esc_passes_prevent_scroll", async () => {
    await openShelf([padOf("auth-notes", AGENT, "x")]);
    const spy = focusCalls();

    await openPad("auth-notes");
    await waitFor(() => expect(drawerPanel().contains(document.activeElement)).toBe(true));
    fireEvent.keyDown(document.body, { key: "Escape" });
    await waitFor(() => expect(drawerPanel().inert).toBe(true));

    expect(keptScroll(spy)).toBe(true);
  });
});
