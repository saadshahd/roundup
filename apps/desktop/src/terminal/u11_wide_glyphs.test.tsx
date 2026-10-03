import * as xtermModule from "@xterm/xterm";
import type { Terminal } from "@xterm/xterm";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createXtermEmulators } from "./emulator";

afterEach(() => vi.restoreAllMocks());

const write = (terminal: Terminal, data: string) => new Promise<void>((resolve) => terminal.write(data, resolve));

/** The real xterm Terminal that `createXtermEmulators` builds, cursor at the origin. */
const realTerminal = (): Terminal => {
  const spy = vi.spyOn(xtermModule, "Terminal");

  createXtermEmulators()("t-a");

  // SAFETY: the one Terminal the spy observed is the instance createXtermEmulators just constructed.
  const terminal = spy.mock.instances[0] as Terminal;

  spy.mockRestore();

  return terminal;
};

describe("u11 wide glyphs take the two cells they draw", () => {
  it.each(["✅", "❌", "🚀"])("u11_wide_%s_advances_the_cursor_by_two_cells", async (glyph) => {
    const terminal = realTerminal();

    await write(terminal, `${glyph}x`);

    expect(terminal.buffer.active.cursorX).toBe(3);
  });

  it("u11_wide_a_narrow_glyph_still_takes_one_cell", async () => {
    const terminal = realTerminal();

    await write(terminal, "ax");

    expect(terminal.buffer.active.cursorX).toBe(2);
  });
});
