import { describe, expect, it } from "vitest";
import { placeOf } from "./places";

const elementIn = (markup: string, selector: string): Element => {
  const host = document.createElement("div");

  host.innerHTML = markup;

  const target = host.querySelector(selector);

  if (!target) throw new Error(`no ${selector} in ${markup}`);

  return target;
};

describe("u41 rail by keyboard, the rest: placeOf", () => {
  it("u41_a_row_in_the_rail_tree_is_the_rail_place", () => {
    const row = elementIn('<div role="tree"><div data-id="a">a</div></div>', "[data-id=a]");

    expect(placeOf(row)).toBe("rail");
  });

  it("u41_the_terminal_pane_is_the_pane_place", () => {
    const screen = elementIn('<div class="pane"><div class="pane-screen"><textarea></textarea></div></div>', "textarea");

    expect(placeOf(screen)).toBe("pane");
  });

  it("u41_the_drawer_is_the_drawer_place", () => {
    const field = elementIn('<aside class="drawer"><input /></aside>', "input");

    expect(placeOf(field)).toBe("drawer");
  });

  it("u41_anything_outside_those_three_is_no_place", () => {
    const span = elementIn("<div><span>loose</span></div>", "span");

    expect(placeOf(span)).toBe("other");
  });

  it("u41_null_is_no_place", () => {
    expect(placeOf(null)).toBe("other");
  });
});
