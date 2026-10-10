import type { ITheme } from "@xterm/xterm";

/** The emulator's colours per scheme, as the values of `tokens.css`'s `--ground`, `--text` and `--selected` in that scheme; `u59_terminal_theme.test.ts` fails when they drift apart. Each ANSI colour is 4.5:1 or better on its scheme's `--ground` as text (U4). */
export const lightTerminalTheme: ITheme = {
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

export const darkTerminalTheme: ITheme = {
  background: "#1c1c1e",
  foreground: "#f5f5f7",
  cursor: "#f5f5f7",
  cursorAccent: "#1c1c1e",
  selectionBackground: "rgba(255, 255, 255, 0.1)",
  black: "#8e8e93",
  red: "#ff6b6b",
  green: "#5fd37a",
  yellow: "#f0a030",
  blue: "#4d9bff",
  magenta: "#d98cff",
  cyan: "#4ccfe0",
  white: "#d1d1d6",
  brightBlack: "#a1a1a6",
  brightRed: "#ff8a8a",
  brightGreen: "#7ee596",
  brightYellow: "#ffbe55",
  brightBlue: "#79b4ff",
  brightMagenta: "#e5a8ff",
  brightCyan: "#7adfec",
  brightWhite: "#ffffff",
};

export const DARK_SCHEME = "(prefers-color-scheme: dark)";

/** The theme for the window's colour scheme. */
export const terminalThemeFor = (dark: boolean): ITheme => (dark ? darkTerminalTheme : lightTerminalTheme);
