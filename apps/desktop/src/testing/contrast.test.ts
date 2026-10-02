import { describe, expect, it } from "vitest";
import { blendBlackOver, contrastRatio, grayscale, resolveToken, tokensOf } from "./contrast";

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
});
