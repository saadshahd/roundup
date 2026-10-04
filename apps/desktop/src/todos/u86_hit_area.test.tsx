import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, beforeEach, expect, it } from "vitest";
import { mountTodos, todo } from "./testHarness";

import styles from "./rowButton.styles.css?inline";

const sheet = document.createElement("style");

// jsdom does not apply :focus-visible; the browser measures keyboard focus with the original selector.
sheet.textContent = styles.replaceAll(":focus-visible", ":focus");

beforeEach(() => document.head.appendChild(sheet));

afterEach(() => {
  cleanup();
  sheet.remove();
});

it("u86_todo_rows_keep_the_hit_area_token_after_the_button_reset", async () => {
  await mountTodos([todo(1), todo(2, { done: true })]);

  const folded = await screen.findByRole("button", { name: "1 done" });
  fireEvent.click(folded);

  for (const row of [
    screen.getByRole("button", { name: "idle #1 todo 1" }),
    folded,
    screen.getByRole("button", { name: "done #2 todo 2" }),
  ]) {
    // jsdom has no layout; the browser proof measures the token's 24px hit height.
    expect(getComputedStyle(row).minHeight).toBe("var(--space-5)");
  }
});

it("u86_todo_rows_allow_a_visible_focus_outline_after_the_button_reset", async () => {
  await mountTodos([todo(1), todo(2, { done: true })]);

  for (const row of [
    await screen.findByRole("button", { name: "idle #1 todo 1" }),
    screen.getByRole("button", { name: "1 done" }),
  ]) {
    row.focus();
    expect(getComputedStyle(row).outline).toBe("calc(var(--space-1) / 2) solid var(--accent)");
    expect(getComputedStyle(row).outlineOffset).toBe("calc(var(--space-1) / 4)");
    expect(row.style.all).toBe("");
  }
});
