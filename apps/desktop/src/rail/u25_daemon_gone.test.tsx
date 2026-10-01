import { cleanup, screen } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import { afterEach, describe, expect, it } from "vitest";
import type { DaemonExit } from "../app/seam";
import { agent, glyphOf, liveLineOf, mountRail, rowNames } from "./railFixture";

afterEach(cleanup);

describe("u25 the Daemon is gone", () => {
  it("u25_the_rail_is_dimmed_and_every_row_keeps_its_glyph_and_live_line", async () => {
    const [exit, setExit] = createSignal<DaemonExit | null>(null);

    await mountRail(
      [agent("a", "needs-you", "approve edit"), agent("b", "working", "reading", { order: 1 })],
      [],
      exit,
    );

    const tree = screen.getByRole("tree");
    const before = { glyph: glyphOf("a").textContent, line: liveLineOf("a") };

    expect(tree.getAttribute("aria-disabled")).toBeNull();
    setExit({ code: 1 });

    expect(tree.getAttribute("aria-disabled")).toBe("true");
    expect(rowNames()).toEqual(["a", "b"]);
    expect({ glyph: glyphOf("a").textContent, line: liveLineOf("a") }).toEqual(before);
  });
});
