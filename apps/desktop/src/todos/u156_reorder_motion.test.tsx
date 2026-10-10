import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import rowCss from "./rowButton.styles.css?inline";
import appCss from "../styles.css?inline";
import { REORDER_EASING, REORDER_MS } from "./reorder";
import { mountTodos, todo, todoEvent } from "./testHarness";

const sheet = document.createElement("style");

sheet.textContent = `${rowCss}\n${appCss}`;

beforeEach(() => {
  document.head.appendChild(sheet);
  vi.spyOn(HTMLElement.prototype, "offsetTop", "get").mockImplementation(function (this: HTMLElement) {
    return this.classList.contains("todo-row") ? [...this.parentElement!.children].indexOf(this) * 30 : 0;
  });
});

afterEach(() => {
  cleanup();
  sheet.remove();
  vi.restoreAllMocks();
  Reflect.deleteProperty(HTMLElement.prototype, "animate");
});

const seed = () => [todo(1), todo(2), todo(3)];

const moveFirstDown = async (reducedMotion: boolean) => {
  const animate = vi.fn();

  Object.assign(HTMLElement.prototype, { animate, getAnimations: () => [] });

  const { app, store } = await mountTodos(seed(), reducedMotion);
  const first = await screen.findByRole("button", { name: /^idle #1 / });

  fireEvent.keyDown(first, { key: "ArrowDown", altKey: true });
  await waitFor(() => expect(store.todos.map((t) => t.id)).toEqual([2, 1, 3]));
  app.emit(todoEvent("todo.updated", { ...store.todos[0]! }));
  await waitFor(() => expect([...document.querySelectorAll("[data-shelf-row]")].map((row) => row.getAttribute("data-id"))).toEqual(["2", "1", "3"]));

  return animate;
};

describe("u156 a moved row's motion", () => {
  it("u156_rows_that_change_place_slide_over_150ms_ease_out_from_where_they_were", async () => {
    const animate = await moveFirstDown(false);

    expect(REORDER_MS).toBe(150);
    expect(REORDER_EASING).toBe("ease-out");
    expect(animate).toHaveBeenCalledTimes(2);
    expect(animate.mock.calls.map(([frames]) => frames)).toEqual([
      [{ transform: "translateY(-30px)" }, { transform: "none" }],
      [{ transform: "translateY(30px)" }, { transform: "none" }],
    ]);
    expect(animate.mock.calls.every(([, options]) => options.duration === 150 && options.easing === "ease-out")).toBe(true);
  });

  it("u156_under_reduced_motion_the_rows_take_their_places_in_one_frame", async () => {
    expect(await moveFirstDown(true)).not.toHaveBeenCalled();
  });

  it("u156_a_hovered_row_has_no_transition_and_the_row_surface_is_instant", async () => {
    await mountTodos(seed());
    await screen.findByText(/todo 3/);

    const row = document.querySelector<HTMLElement>("[data-id='1']")!;

    fireEvent.mouseEnter(row);
    expect(["", "0s", "all 0s ease 0s"]).toContain(getComputedStyle(row).transitionDuration || "");
    expect(getComputedStyle(row).transitionProperty === "" || getComputedStyle(row).transitionProperty === "all").toBe(true);
  });
});
