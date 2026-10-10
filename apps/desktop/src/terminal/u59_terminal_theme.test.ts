import { describe, expect, it, vi } from "vitest";
import tokenStyles from "../tokens.css?inline";
import terminalStyles from "./styles.css?inline";
import { blendBlackOver, contrastRatio, loadTokens, resolveToken, tokensOf } from "../testing/contrast";
import { xtermOptions } from "./emulator";
import { darkTerminalTheme, followScheme, lightTerminalTheme } from "./theme";

const ANSI = [
  "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
  "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue", "brightMagenta", "brightCyan", "brightWhite",
] as const;

const darkRoot = /@media \(prefers-color-scheme: dark\)\s*\{(\s*:root\s*\{[^}]*\})/.exec(tokenStyles)![1]!;

const blendWhiteOver = (hex: string, alpha: number): string => {
  const mix = (at: number) => Math.round(parseInt(hex.slice(at, at + 2), 16) * (1 - alpha) + 255 * alpha).toString(16).padStart(2, "0");

  return `#${mix(1)}${mix(3)}${mix(5)}`;
};

describe("u59 the terminal blends with the app", () => {
  const light = loadTokens();
  const lightToken = (name: string) => resolveToken(`var(--${name})`, light.tokens);
  const dark = new Map([...light.tokens, ...tokensOf(darkRoot)]);
  const darkToken = (name: string) => resolveToken(`var(--${name})`, dark);

  const schemes = [
    { scheme: "light", theme: lightTerminalTheme, token: lightToken, ground: light.ground, wash: (g: string) => blendBlackOver(g, 0.05) },
    { scheme: "dark", theme: darkTerminalTheme, token: darkToken, ground: darkToken("ground"), wash: (g: string) => blendWhiteOver(g, 0.1) },
  ] as const;

  it("u59_the_emulator_starts_from_the_light_theme", () => {
    expect(xtermOptions.theme).toBe(lightTerminalTheme);
  });

  it.each(schemes)("u59_the_$scheme_theme_is_built_from_the_tokens", ({ theme, token, ground }) => {
    expect(theme.background).toBe(ground);
    expect(theme.cursorAccent).toBe(ground);
    expect(theme.foreground).toBe(token("text"));
    expect(theme.cursor).toBe(token("text"));
    expect(theme.selectionBackground).toBe(token("selected").replace(/,(?=\S)/g, ", "));
  });

  it("u59_the_dark_theme_is_not_the_light_one", () => {
    expect(darkTerminalTheme.background).not.toBe(lightTerminalTheme.background);
  });

  it.each(schemes.flatMap((entry) => ANSI.map((name) => ({ ...entry, name }))))("u59_ansi_$name_reads_at_4_5_to_1_on_the_$scheme_ground", ({ theme, name, ground }) => {
    const colour = theme[name]!;

    expect(contrastRatio(colour, ground), `${name} ${colour} on ${ground}`).toBeGreaterThanOrEqual(4.5);
  });

  it.each(schemes)("u59_the_selection_band_keeps_the_text_readable_in_$scheme", ({ theme, ground, wash }) => {
    expect(contrastRatio(theme.foreground!, wash(ground))).toBeGreaterThanOrEqual(4.5);
  });

  it("u59_the_emulator_repaints_when_the_scheme_changes", () => {
    let matches = false;
    let notify = () => {};

    vi.stubGlobal("matchMedia", () => ({
      get matches() { return matches; },
      addEventListener: (_: string, listener: () => void) => { notify = listener; },
      removeEventListener: () => { notify = () => {}; },
    }));
    const seen: unknown[] = [];
    const stop = followScheme((theme) => seen.push(theme));

    matches = true;
    notify();
    stop();
    vi.unstubAllGlobals();

    expect(seen).toEqual([lightTerminalTheme, darkTerminalTheme]);
  });

  it("u59_the_font_is_the_rails_monospace_face_at_the_rails_size", () => {
    expect(xtermOptions.fontFamily).toBe(lightToken("font-mono"));
    expect(xtermOptions.fontSize).toBe(Number.parseInt(lightToken("text-body"), 10));
  });

  it("u59_the_stylesheet_overrides_xterms_black_viewport_with_the_ground", () => {
    expect(terminalStyles).toMatch(/\.xterm \.xterm-viewport\s*\{\s*background-color:\s*var\(--ground\);/);
  });

  it("u59_the_pane_has_no_border", () => {
    expect(terminalStyles).not.toMatch(/\.pane[^{]*\{[^}]*border\s*:/);
  });
});
