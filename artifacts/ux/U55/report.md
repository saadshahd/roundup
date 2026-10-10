# U55 report: the empty-Terminal notch

**Which element.** xterm's block cursor, at the first cell of the first row. It is not a measure element and not the pane's own background.

- On `9784d54` (the commit U55 names; headless Chromium, `tree-40`, 1280x800, the loose `zsh` Terminal selected, no output): the pane has no theme, so xterm uses its defaults, `background #000000` and `cursor #ffffff`. The renderer is canvas: `.xterm-screen` holds two `<canvas>` (`.xterm-link-layer` and an unclassed text layer), each 798x720 at (272, 70), and no cursor element. The white cell is the cursor drawn on the text layer at column 0, row 0: one cell, 7 wide and 15 tall, at x 272 to 279, y 70 to 85.
- Proof it is the cursor: after one line of output (`hello`) the notch is gone from row 0 and shows at the start of row 1 (`after-output.png`).
- On `main` `d0ef55a` (U59's theme: `background #ffffff`, `cursor #1d1d1f`, `cursorAccent #ffffff`, `apps/desktop/src/terminal/theme.ts`) the same cell is a `span.xterm-cursor.xterm-cursor-block` in `.xterm-rows`, computed 7.84375 x 15 px, `background-color rgb(29, 29, 31)`, `color rgb(255, 255, 255)`, set by xterm's DOM renderer's `.xterm-cursor-block` rule from the theme. It sits at (272, 40) (U57 removed the header). It is a dark cell on white, and `main-after-output.png` shows it moving to row 1 the same way.

**Rule that sets size and colour.** Size is the cell: the width is xterm's measured char width (`.xterm-char-measure-element`, 7.84375 px at 13px `ui-monospace`) and the height is the 15px row (`line-height: 15px`). Colour is `theme.cursor` (default `#ffffff`, U59's `#1d1d1f`). `xtermOptions` in `apps/desktop/src/terminal/emulator.ts:70` sets no `cursorBlink`, `cursorStyle` or `cursorInactiveStyle`, so it is a steady block.

**Window sizes.** All of them: the cursor is always 7.84 x 15 px at the Terminal's top-left. Checked at 700 (x 268), 1280 (x 272) and 1920 (x 336) px wide; the notch shows at all three while a Terminal has produced no output (`notch-700.png`, `notch.png`, `notch-1920.png`). The cell size does not change with width; only the origin follows the Rail.

**For the Architect.** The notch is the intended cursor of a Terminal with no output, not a stray element. On `main` it is dark on white, no longer a white notch on black. If it should not show before the first byte, the fix is a cursor option (`cursorInactiveStyle`, or hiding the cursor until output), not a layout change.

Files: `notch.png` (circled), `after-output.png`, `notch-700.png`, `notch-1920.png` (all `9784d54`); `main-notch.png`, `main-after-output.png` (`main`).
