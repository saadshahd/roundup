import { cleanup, fireEvent, render } from "@solidjs/testing-library";
import { afterEach, expect, it } from "vitest";
import { KindGlyph } from "../ink/KindGlyph";
import { agent, workstream } from "../testing/nodes";
import { mountRail, rowOf } from "./railFixture";

afterEach(cleanup);

it("u3704_marked_kind_glyphs_and_counts", async () => {
  const { container } = render(() => <><KindGlyph kind="error" /><KindGlyph kind="idle" decorative /></>);
  const [shown, decorative] = [...container.querySelectorAll(".glyph")];

  expect([shown?.getAttribute("data-state"), decorative?.hasAttribute("data-state")]).toEqual(["error", false]);
  cleanup();

  await mountRail([
    workstream("open", { name: "open ws", order: 0 }),
    agent("o1", "needs-you", "asks", { parent: "open", name: "ask-1" }),
    workstream("shut", { name: "shut ws", order: 1 }),
    agent("s1", "idle", "i", { parent: "shut", name: "idle-1" }),
  ]);

  expect(rowOf("open ws").querySelector(".ink-count")?.hasAttribute("data-state")).toBe(true);
  fireEvent.click(rowOf("shut ws").querySelector('button[aria-label="Collapse"]')!);
  expect(rowOf("shut ws").querySelector(".folded-agents")?.hasAttribute("data-state")).toBe(true);
  expect(rowOf("ask-1").querySelector(".glyph")?.hasAttribute("data-state")).toBe(true);

  for (const state of document.querySelectorAll(".glyph[data-state], .ink-count[data-state], .folded-agents[data-state]")) {
    expect(state.matches("button, a, input, [role=button]")).toBe(false);
    expect(state.getAttribute("tabindex") ?? "-1").toBe("-1");
  }
});
