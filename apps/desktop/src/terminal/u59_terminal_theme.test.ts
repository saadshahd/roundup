import { describe, expect, it } from "vitest";
import terminalStyles from "./styles.css?inline";
import { blendBlackOver, contrastRatio, loadTokens, resolveToken } from "../testing/contrast";
import { xtermOptions } from "./emulator";
import { terminalTheme } from "./theme";

const ANSI = [
  "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
  "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue", "brightMagenta", "brightCyan", "brightWhite",
] as const;

describe("u59 the terminal blends with the app", () => {
  const { tokens, ground } = loadTokens();
  const token = (name: string) => resolveToken(`var(--${name})`, tokens);

  it("u59_the_emulator_theme_is_built_from_the_tokens", () => {
    expect(xtermOptions.theme).toBe(terminalTheme);
    expect(terminalTheme.background).toBe(ground);
    expect(terminalTheme.foreground).toBe(token("text"));
    expect(terminalTheme.cursor).toBe(token("text"));
    expect(terminalTheme.selectionBackground).toBe(token("selected").replace(/,(?=\S)/g, ", "));
  });

  it("u59_the_font_is_the_rails_monospace_face_at_the_rails_size", () => {
    expect(xtermOptions.fontFamily).toBe(token("font-mono"));
    expect(xtermOptions.fontSize).toBe(Number.parseInt(token("text-body"), 10));
  });

  it.each(ANSI)("u59_ansi_%s_reads_at_4_5_to_1_on_the_ground", (name) => {
    const colour = terminalTheme[name]!;

    expect(contrastRatio(colour, ground), `${name} ${colour}`).toBeGreaterThanOrEqual(4.5);
  });

  it("u59_the_selection_band_keeps_the_text_readable", () => {
    expect(contrastRatio(terminalTheme.foreground!, blendBlackOver(ground, 0.05))).toBeGreaterThanOrEqual(4.5);
  });

  it("u59_the_stylesheet_overrides_xterms_black_viewport_with_the_ground", () => {
    expect(terminalStyles).toMatch(/\.xterm \.xterm-viewport\s*\{\s*background-color:\s*var\(--ground\);/);
  });

  it("u59_the_pane_has_no_border", () => {
    expect(terminalStyles).not.toMatch(/\.pane[^{]*\{[^}]*border\s*:/);
  });
});
