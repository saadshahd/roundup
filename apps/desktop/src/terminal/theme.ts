import type { ITheme } from "@xterm/xterm";

/** The emulator's colours, as the values of `tokens.css`'s `--ground`, `--text` and `--selected`; `u59_terminal_theme.test.ts` fails when they drift apart. The ANSI colours are a light palette, each at 4.5:1 or better on `--ground` as text (U4). */
export const terminalTheme: ITheme = {
  background: "#ffffff",
  foreground: "#1d1d1f",
  cursor: "#1d1d1f",
  cursorAccent: "#ffffff",
  selectionBackground: "rgba(0, 0, 0, 0.05)",
  black: "#1d1d1f",
  red: "#c4262e",
  green: "#1a7f37",
  yellow: "#975900",
  blue: "#0a60d8",
  magenta: "#a626a4",
  cyan: "#0b7285",
  white: "#68686d",
  brightBlack: "#515154",
  brightRed: "#d1242f",
  brightGreen: "#167a33",
  brightYellow: "#8f5400",
  brightBlue: "#0a55c0",
  brightMagenta: "#9b2199",
  brightCyan: "#0a6a7c",
  brightWhite: "#3a3a3c",
};
