import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import rowCss from "./rowButton.styles.css?inline";
import appCss from "../styles.css?inline";
import { mountTodos, todo, todoRowOf as rowOf } from "./testHarness";

const sheet = document.createElement("style");

sheet.textContent = `${rowCss}\n${appCss}`;

beforeEach(() => document.head.appendChild(sheet));

afterEach(() => {
  cleanup();
  sheet.remove();
});

const seed = () => [todo(1), todo(2, { blocked: true, blockers: [1] }), todo(3)];

const style = (el: Element) => getComputedStyle(el);

describe("u153 the row surface and rhythm", () => {
  it("u153_a_row_is_a_ground_surface_with_the_row_radius_and_token_padding", async () => {
    await mountTodos(seed());
    await screen.findByText(/todo 3/);

    for (const id of [1, 2]) {
      const row = style(rowOf(id));

      expect([row.backgroundColor, row.borderRadius, row.paddingTop, row.paddingBottom, row.paddingLeft, row.paddingRight]).toEqual([
        "var(--ground)",
        "var(--radius-row)",
        "var(--space-1)",
        "var(--space-1)",
        "var(--space-2)",
        "var(--space-2)",
      ]);
      expect([row.borderStyle, row.boxShadow]).toEqual([expect.stringMatching(/^(|none)$/), expect.stringMatching(/^(|none)$/)]);
    }
  });

  it("u153_rows_are_a_space_1_apart_and_the_list_pads_a_space_2", async () => {
    await mountTodos(seed());
    await screen.findByText(/todo 3/);

    const list = rowOf(1).parentElement!;

    expect([style(list).display, style(list).flexDirection, style(list).rowGap]).toEqual(["flex", "column", "var(--space-1)"]);
    expect([style(list).paddingTop, style(list).paddingBottom, style(list).paddingLeft, style(list).paddingRight]).toEqual(Array(4).fill("var(--space-2)"));
  });

  it("u153_the_row_button_is_at_least_28px_and_body_text", async () => {
    await mountTodos(seed());

    const button = await screen.findByRole("button", { name: /^idle #1 / });

    expect([style(button).minHeight, style(button).fontSize, style(button).color]).toEqual(["28px", "var(--text-body)", "var(--text)"]);
  });

  it("u153_the_waits_on_and_home_lines_are_caption_grey_and_indented_one_space_step", async () => {
    await mountTodos(seed());
    await screen.findByText(/todo 3/);

    for (const line of [rowOf(2).querySelector("[data-todo-waits]")!, rowOf(2).querySelector("[data-todo-home]")!]) {
      expect([style(line).fontSize, style(line).color, style(line).paddingLeft]).toEqual(["var(--text-caption)", "var(--grey)", expect.stringMatching(/^(var\(--space-4\)|16px)$/)]);
    }
  });

  it("u153_hover_and_selected_wash_the_ground_at_once_with_no_transition", async () => {
    await mountTodos(seed());
    await screen.findByText(/todo 3/);

    const row = rowOf(1);

    expect(style(row).backgroundImage).toBe("none");
    expect(style(row).transitionDuration).toMatch(/^(|0s)$/);

    fireEvent.click(await screen.findByRole("button", { name: /^idle #1 / }));
    await screen.findByRole("complementary", { name: "drawer" });

    expect(row.getAttribute("data-selected")).toBe("true");
    expect(style(row).backgroundImage).toBe("linear-gradient(var(--selected), var(--selected))");
    expect(rowOf(3).getAttribute("data-selected")).toBeNull();
    expect(style(row).backgroundColor).toBe("var(--ground)");
  });
});
