import type { ITheme } from "@xterm/xterm";

/** The emulator's colours in the light scheme, as the values of `tokens.css`'s `--ground`, `--text` and `--selected`; `u59_terminal_theme.test.ts` fails when they drift apart. The ANSI colours are a light palette, each at 4.5:1 or better on `--ground` as text (U4). */
export const lightTerminalTheme: ITheme = {
  background: "#ffffff",
  foreground: "#1d1d1f",
  cursor: "#1d1d1f",
  cursorAccent: "#ffffff",
  selectionBackground: "rgba(0, 0, 0, 0.05)",
  black: "#1d1d1f",
  red: "#c4262e",
  green: "#1a7f37",
  yellow: "#9a5b00",
  blue: "#0a60d8",
  magenta: "#a626a4",
  cyan: "#0b7285",
  white: "#6e6e73",
  brightBlack: "#515154",
  brightRed: "#d1242f",
  brightGreen: "#167a33",
  brightYellow: "#8f5400",
  brightBlue: "#0a55c0",
  brightMagenta: "#9b2199",
  brightCyan: "#0a6a7c",
  brightWhite: "#3a3a3c",
};

/** The same fields in the dark scheme, as the values of `tokens.css`'s dark `--ground`, `--text` and `--selected`; the ANSI colours are a dark palette, each at 4.5:1 or better on that `--ground` as text (U4). */
export const darkTerminalTheme: ITheme = {
  background: "#1c1c1e",
  foreground: "#f5f5f7",
  cursor: "#f5f5f7",
  cursorAccent: "#1c1c1e",
  selectionBackground: "rgba(255, 255, 255, 0.1)",
  black: "#8e8e93",
  red: "#ff6b6b",
  green: "#4cd964",
  yellow: "#f0a030",
  blue: "#4d9bff",
  magenta: "#d67ad8",
  cyan: "#4dc9d9",
  white: "#d1d1d6",
  brightBlack: "#a1a1a6",
  brightRed: "#ff8a8a",
  brightGreen: "#6ee27f",
  brightYellow: "#ffc266",
  brightBlue: "#7ab4ff",
  brightMagenta: "#e59be7",
  brightCyan: "#7fdbe8",
  brightWhite: "#ffffff",
};

const DARK_QUERY = "(prefers-color-scheme: dark)";

/** The theme for the webview's current scheme. */
export const terminalThemeFor = (dark: boolean): ITheme => (dark ? darkTerminalTheme : lightTerminalTheme);

/** Calls `apply` with the current scheme's theme now and again whenever the scheme changes; returns the stop function. Without `matchMedia` the scheme is light. */
export const followScheme = (apply: (theme: ITheme) => void): (() => void) => {
  if (!globalThis.matchMedia) {
    apply(lightTerminalTheme);

    return () => {};
  }

  const query = matchMedia(DARK_QUERY);
  const update = () => apply(terminalThemeFor(query.matches));

  update();
  query.addEventListener?.("change", update);

  return () => query.removeEventListener?.("change", update);
};
