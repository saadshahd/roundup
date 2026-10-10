import { describe, expect, it } from "vitest";
import terminalStyles from "./styles.css?inline";
import { contrastRatio, loadTokens, resolveToken } from "../testing/contrast";
import { xtermOptions } from "./emulator";
import tokenStyles from "../tokens.css?inline";
import { darkTerminalTheme, lightTerminalTheme, terminalThemeFor } from "./theme";
import type { ITheme } from "@xterm/xterm";

const ANSI = [
  "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
  "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue", "brightMagenta", "brightCyan", "brightWhite",
] as const;

const darkBlock = /@media \(prefers-color-scheme: dark\)\s*\{\s*:root\s*\{([^}]*)\}/.exec(tokenStyles)![1]!;

const darkOverrides = new Map([...darkBlock.matchAll(/--([\w-]+):\s*([^;]+);/g)].map((m) => [m[1]!, m[2]!.trim()]));


describe("u59 the terminal blends with the app", () => {
  const light = loadTokens();

  const dark = new Map([...light.tokens, ...darkOverrides]);

  const schemes: [string, ITheme, ReadonlyMap<string, string>][] = [
    ["light", lightTerminalTheme, light.tokens],
    ["dark", darkTerminalTheme, dark],
  ];

  const token = (name: string) => resolveToken(`var(--${name})`, light.tokens);

  it("u59_the_emulator_theme_follows_the_scheme", () => {
    expect(terminalThemeFor(false)).toBe(lightTerminalTheme);
    expect(terminalThemeFor(true)).toBe(darkTerminalTheme);
    expect(xtermOptions.theme).toBe(lightTerminalTheme);
  });

  describe.each(schemes)("%s", (_name, theme, tokens) => {
    const resolve = (name: string) => resolveToken(`var(--${name})`, tokens);
    const ground = resolve("ground");

    it("u59_the_emulator_theme_is_built_from_the_tokens", () => {
      expect(theme.background).toBe(ground);
      expect(theme.foreground).toBe(resolve("text"));
      expect(theme.cursor).toBe(resolve("text"));
      expect(theme.selectionBackground).toBe(resolve("selected").replace(/,(?=\S)/g, ", "));
    });

    it.each(ANSI)("u59_ansi_%s_reads_at_4_5_to_1_on_the_ground", (name) => {
      const colour = theme[name]!;

      expect(contrastRatio(colour, ground), `${name} ${colour}`).toBeGreaterThanOrEqual(4.5);
    });

    it("u59_the_selection_band_keeps_the_text_readable", () => {
      const [r, , , a] = [...theme.selectionBackground!.matchAll(/[\d.]+/g)].map((m) => Number(m[0]));
      const wash = (channel: number) => Math.round(channel * (1 - a!) + r! * a!).toString(16).padStart(2, "0");
      const hex = (c: string) => [1, 3, 5].map((i) => Number.parseInt(c.slice(i, i + 2), 16));
      const band = `#${hex(ground).map(wash).join("")}`;

      expect(contrastRatio(theme.foreground!, band)).toBeGreaterThanOrEqual(4.5);
    });
  });

  it("u59_the_font_is_the_rails_monospace_face_at_the_rails_size", () => {
    expect(xtermOptions.fontFamily).toBe(token("font-mono"));
    expect(xtermOptions.fontSize).toBe(Number.parseInt(token("text-body"), 10));
  });

  it("u59_the_stylesheet_overrides_xterms_black_viewport_with_the_ground", () => {
    expect(terminalStyles).toMatch(/\.xterm \.xterm-viewport\s*\{\s*background-color:\s*var\(--ground\);/);
  });

  it("u59_the_pane_has_no_border", () => {
    expect(terminalStyles).not.toMatch(/\.pane[^{]*\{[^}]*border\s*:/);
  });
});
