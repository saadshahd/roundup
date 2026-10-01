import { cleanup, fireEvent, screen, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { openShelf, padOf, YOU } from "./padsFixture";

afterEach(cleanup);

const LONG = "a-pad-with-a-very-long-name-here-ok";

const cut = (name: HTMLElement) => {
  const style = getComputedStyle(name);

  return [
    style.minWidth,
    style.overflow,
    style.textOverflow,
    style.whiteSpace,
    name.title,
  ];
};

describe("u24 long names", () => {
  it("u24_a_long_pad_name_keeps_the_mark_and_the_name_on_one_cut_line", async () => {
    await openShelf([padOf(LONG, YOU)]);
    const name = await screen.findByText(LONG);
    const row = name.closest("p");

    expect(row && getComputedStyle(row).display).toBe("flex");
    expect(row?.contains(screen.getByText("◇"))).toBe(true);
    expect(cut(name)).toEqual(["0px", "hidden", "ellipsis", "nowrap", LONG]);
  });

  it("u24_the_drawer_first_line_cuts_a_long_name_and_leaves_the_owner_words_whole", async () => {
    await openShelf([padOf(LONG, YOU)]);
    fireEvent.click(await screen.findByText(LONG));
    const name = await within(await screen.findByLabelText("drawer")).findByText(LONG);
    const row = name.closest("p");

    expect(cut(name)).toEqual(["0px", "hidden", "ellipsis", "nowrap", LONG]);
    expect(row && getComputedStyle(row).display).toBe("flex");
    expect(row?.textContent).toContain("owned by");
    expect(row && getComputedStyle(within(row).getByText(/^owned by/)).flexShrink).toBe("0");
  });
});
