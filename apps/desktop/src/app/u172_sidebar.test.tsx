import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { mountTodos, todo } from "../todos/testHarness";
import { agent, terminal, workstream } from "../testing/nodes";
import { mountRail, rowOf } from "../rail/railFixture";
import { seedApp } from "../testing/seeds";
import type { SeedName } from "../testing/seeds";
import tokenCss from "../tokens.css?inline";
import appCss from "../styles.css?inline";
import railCss from "../rail/styles.css?inline";
import buttonCss from "../buttons.css?inline";
import rowCss from "../todos/rowButton.styles.css?inline";

let sheet: HTMLStyleElement | undefined;

afterEach(() => {
  sheet?.remove();
  cleanup();
  localStorage.clear();
});

const styled = () => {
  sheet = document.head.appendChild(document.createElement("style"));
  sheet.textContent = [tokenCss, appCss, railCss, buttonCss, rowCss].join("\n");
};

const mountApp = async (seed: SeedName) => {
  window.matchMedia ??= (query) => ({ matches: false, media: query, onchange: null, addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {}, dispatchEvent: () => false });
  styled();
  const controls = seedApp(seed, Date.now());

  render(() => <App app={controls.app} reducedMotion={() => true} clock={Date.now} />);
  await screen.findAllByRole("treeitem");

  return controls;
};

const workstreams = () => screen.getAllByRole("treeitem").filter((row) => row.hasAttribute("aria-expanded"));

const head = (group: string) => document.querySelector<HTMLElement>(`[data-group="${group}"]`)!;

describe("u172 the sidebar's hierarchy", () => {
  it("u172_a_workstream_row_reads_badge_name_chevron_value", async () => {
    await mountApp("tree-40");
    const rows = workstreams();

    expect(rows.length).toBeGreaterThan(0);

    for (const row of rows) {
      const parts = [...row.querySelector(".line")!.children];
      const name = row.querySelector(".name")!.textContent!;
      const badge = row.querySelector(".badge")!;
      const order = ["badge", "name", "button", "value"].map((part) => parts.findIndex((el) => el.classList.contains(part) || el.tagName.toLowerCase() === part));

      expect(badge.textContent).toBe([...name.trim()].slice(0, 2).join("").toUpperCase());
      expect(order.every((at) => at >= 0), name).toBe(true);
      expect(order, name).toEqual([...order].sort((a, b) => a - b));
      expect(row.querySelector('button[aria-label="collapse"]')!.getAttribute("data-button")).toBe("icon");
      expect(Number(row.querySelector(".value")!.textContent), name).toBeGreaterThanOrEqual(0);
    }

    expect(getComputedStyle(workstreams()[0]!.querySelector(".badge")!).borderRadius).toBe("var(--radius-row)");
    expect(getComputedStyle(workstreams()[0]!.querySelector(".badge")!).fontSize).toBe("var(--text-caption)");
  });

  it("u172_a_workstream_value_counts_its_agents_that_are_not_done", async () => {
    await mountRail([workstream("g"), agent("a", "working", "w", { parent: "g" }), agent("b", "done", "d", { parent: "g" }), agent("c", "idle", "i", { parent: "g" }), terminal("t", { parent: "g" })]);

    expect(rowOf("g").querySelector(".value")!.textContent).toBe("2");
  });

  it("u172_each_shelf_list_has_one_head_with_a_mark_a_label_a_rule_and_a_count", async () => {
    styled();
    await mountTodos([todo(1, { body: "first" }), todo(2, { done: true })]);
    const section = screen.getByRole("region", { name: "todos" });
    const group = head("shelf:todos");

    expect([...group.children].map((el) => el.className.split(" ").filter((name) => name.startsWith("group-"))[0])).toEqual(["group-toggle", "group-rule", "group-count", undefined]);
    expect(group.querySelector("svg.icon")!.getAttribute("width")).toBe("16");
    expect(group.querySelector(".group-label")!.textContent).toBe("todos");
    expect(getComputedStyle(group.querySelector(".group-label")!).textTransform).toBe("uppercase");
    expect(getComputedStyle(group.querySelector(".group-rule")!).borderTopColor).toBe("var(--hairline)");
    expect(group.querySelector(".group-count")!.textContent).toMatch(/^\d+\/\d+$/);
    expect(within(section).getByRole("button", { name: "add todo" }).getAttribute("data-button")).toBe("icon");
  });

  it("u172_collapsing_a_list_hides_its_rows_keeps_its_head_and_count_and_survives_a_remount", async () => {
    const first = await mountApp("tree-40");
    const toggle = () => within(head("shelf:todos")).getByRole("button", { name: /todos/ });

    await waitFor(() => expect(document.querySelectorAll("[data-todo]").length).toBeGreaterThan(0));
    const count = head("shelf:todos").querySelector(".group-count")!.textContent;
    fireEvent.click(toggle());

    expect([toggle().getAttribute("aria-expanded"), document.querySelectorAll('section[aria-label="todos"] [data-todo]').length, head("shelf:todos").querySelector(".group-count")!.textContent]).toEqual(["false", 0, count]);
    expect(JSON.parse(localStorage.getItem(Object.keys(localStorage).find((key) => key.startsWith("roundup:rail:"))!)!).collapsed).toContain("shelf:todos");

    cleanup();
    render(() => <App app={first.app} reducedMotion={() => true} clock={Date.now} />);
    await screen.findAllByRole("treeitem");

    expect(toggle().getAttribute("aria-expanded")).toBe("false");
    expect(document.querySelectorAll('section[aria-label="todos"] [data-todo]').length).toBe(0);
  });

  it("u172_an_item_shows_a_grey_two_line_description_under_its_bold_title", async () => {
    styled();
    await mountTodos([todo(1, { title: "ship", body: "line one of the note" })]);
    const row = await screen.findByRole("button", { name: /#1 ship/ });
    const description = document.querySelector(".item-description")!;

    expect(description.textContent).toBe("line one of the note");
    expect(getComputedStyle(row).fontWeight).toBe("600");
    expect([getComputedStyle(description).color, getComputedStyle(description).fontSize, getComputedStyle(description).overflow]).toEqual(["var(--grey)", "var(--text-caption)", "hidden"]);
    expect(row.compareDocumentPosition(description) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(document.querySelector("kbd")).toBeNull();
  });

  it("u172_an_item_with_no_body_draws_no_description", async () => {
    styled();
    await mountTodos([todo(1)]);

    expect(document.querySelector(".item-description")).toBeNull();
  });

  it("u172_one_selected_row_has_the_band_and_the_accent_bar_as_a_background_never_a_border", async () => {
    await mountApp("agents-10");
    const rows = screen.getAllByRole("treeitem");
    fireEvent.click(rows[1]!);

    const marked = screen.getAllByRole("treeitem").filter((row) => getComputedStyle(row).backgroundImage.includes("--accent"));
    const style = getComputedStyle(marked[0]!);

    expect(marked.map((row) => row.getAttribute("aria-selected"))).toEqual(["true"]);
    expect(railCss).toMatch(/\.rail-row\[data-selected="true"\]\s*\{[^}]*background:\s*var\(--selected\)/);
    expect([style.backgroundSize, style.borderLeftWidth]).toEqual(["2px 100%", "0px"]);
  });

  it("u172_a_block_is_told_from_the_one_above_by_a_hairline_and_space_3", async () => {
    await mountApp("tree-40");
    const second = workstreams()[1]!;
    const style = getComputedStyle(second);

    expect([style.borderTopWidth, style.borderTopColor, style.marginTop]).toEqual(["1px", "var(--hairline)", "var(--space-3)"]);
    expect(getComputedStyle(workstreams()[0]!).borderTopStyle).toBe("none");
  });

  it("u172_rows_have_at_least_space_2_vertical_and_space_4_horizontal_padding_in_the_rail", async () => {
    await mountApp("tree-40");
    const rail = getComputedStyle(document.querySelector(".rail")!);

    expect([rail.paddingLeft, rail.paddingRight]).toEqual(["var(--space-4)", "var(--space-4)"]);
    expect(getComputedStyle(workstreams()[0]!).minHeight).toBe("28px");
  });
});
