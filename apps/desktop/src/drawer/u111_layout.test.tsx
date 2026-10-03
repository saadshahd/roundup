import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import styles from "../styles.css?inline";
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

describe("u111 opening and closing a Drawer leaves the layout where it was", () => {
  it("u111_the_element_holding_the_rail_the_terminal_and_the_shelf_clips_sideways", async () => {
    const overflowX = await withStylesheet(async () => {
      await mountTodos([todo(1)]);

      return getComputedStyle(document.querySelector(".columns")!).overflowX;
    });

    expect(overflowX).toBe("clip");
  });

  it("u111_every_focus_into_or_out_of_the_drawer_passes_prevent_scroll", async () => {
    const spy = focusCalls();
    const { connected } = await mountTodos([todo(1)]);

    connected.drawer.open(() => <input aria-label="field" />);
    await waitFor(() => expect(screen.getByLabelText("drawer").contains(document.activeElement)).toBe(true));
    connected.drawer.close();
    await waitFor(() => expect((screen.getByLabelText("drawer") as HTMLElement & { inert: boolean }).inert).toBe(true));

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
    await waitFor(() => expect((screen.getByLabelText("drawer") as HTMLElement & { inert: boolean }).inert).toBe(true));

    const options = spy.mock.calls.map(([option]) => option);
    spy.mockRestore();
    outside.remove();

    expect(options.length).toBeGreaterThan(0);
    expect(options.every((option) => option?.preventScroll === true)).toBe(true);
  });
});
