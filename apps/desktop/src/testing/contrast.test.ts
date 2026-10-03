import { describe, expect, it } from "vitest";
import { applyFilter, blendBlackOver, contrastRatio, grayscale, greyedRailFilter, resolveToken, tokensFrom, tokensOf } from "./contrast";

describe("contrast", () => {
  it("contrastRatio_black_on_white_is_21_to_1", () => {
    expect(contrastRatio("#000000", "#ffffff")).toBeCloseTo(21, 0);
  });

  it("contrastRatio_a_colour_against_itself_is_1_to_1", () => {
    expect(contrastRatio("#6e6e73", "#6e6e73")).toBeCloseTo(1, 5);
  });

  it("contrastRatio_does_not_care_which_argument_is_lighter", () => {
    expect(contrastRatio("#1c1c1e", "#ffffff")).toBeCloseTo(contrastRatio("#ffffff", "#1c1c1e"), 10);
  });

  it("blendBlackOver_a_5_percent_wash_on_white_matches_the_selected_row_band", () => {
    expect(blendBlackOver("#ffffff", 0.05)).toBe("#f2f2f2");
  });

  it("grayscale_a_near_neutral_grey_is_almost_unchanged", () => {
    expect(grayscale("#6e6e73")).toBe("#6e6e6e");
  });

  it("tokensOf_reads_root_custom_properties_out_of_stylesheet_text", () => {
    const tokens = tokensOf(":root {\n  --ground: #ffffff;\n  --grey: #6e6e73;\n}\n.other { color: red; }");

    expect([tokens.get("ground"), tokens.get("grey")]).toEqual(["#ffffff", "#6e6e73"]);
  });

  it("tokensFrom_lets_a_later_sheet_win_a_name_both_define", () => {
    const tokens = tokensFrom(":root {\n  --x: #111111;\n}", ":root {\n  --x: #222222;\n  --y: #333333;\n}");

    expect([tokens.get("x"), tokens.get("y")]).toEqual(["#222222", "#333333"]);
  });

  it("tokensOf_ignores_a_token_with_no_value", () => {
    const tokens = tokensOf(":root { --x: ; --y: #000000; }");

    expect(tokens.has("x")).toBe(false);
    expect(tokens.get("y")).toBe("#000000");
  });

  it("resolveToken_follows_a_chain_of_var_references_to_a_literal", () => {
    const tokens = new Map([
      ["live", "var(--grey)"],
      ["grey", "#6e6e73"],
    ]);

    expect(resolveToken("var(--live)", tokens)).toBe("#6e6e73");
  });

  it("resolveToken_a_literal_value_passes_through_unchanged", () => {
    expect(resolveToken("#c4262e", new Map())).toBe("#c4262e");
  });

  it("applyFilter_grayscale_1_matches_the_grayscale_helper", () => {
    expect(applyFilter("#9a5b00", "grayscale(1)")).toBe(grayscale("#9a5b00"));
  });

  it("applyFilter_contrast_pulls_every_channel_toward_mid_grey", () => {
    expect(applyFilter("#ffffff", "contrast(0.4)")).toBe("#b3b3b3");
  });

  it("applyFilter_runs_functions_in_the_order_the_declaration_lists_them", () => {
    const sequential = applyFilter(applyFilter("#9a5b00", "grayscale(1)"), "contrast(0.4)");

    expect(applyFilter("#9a5b00", "grayscale(1) contrast(0.4)")).toBe(sequential);
  });

  it("greyedRailFilter_reads_the_filter_declaration_off_the_aria_disabled_rule", () => {
    const css = '.rail-tree [role="tree"][aria-disabled="true"] {\n  filter: grayscale(1);\n}';

    expect(greyedRailFilter(css)).toBe("grayscale(1)");
  });
});
