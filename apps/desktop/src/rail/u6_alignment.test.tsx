import { cleanup, fireEvent, screen, within } from "@solidjs/testing-library";
import { afterEach, expect, it } from "vitest";
import { withStylesheets } from "../testing/contrast";
import { agent, room, door, terminal } from "../testing/nodes";
import { exitedTerminal, mountRail } from "./railFixture";

afterEach(cleanup);

it.each([0, 1, 2])("u6_row_contents_stay_centered_in_the_selection_band_at_depth_%i", async (depth) => {
  await withStylesheets(async () => {
    const kinds = ["error", "needs-you", "blocked", "working", "idle", "done"] as const;
    const parent = depth === 0 ? null : `home-${depth - 1}`;

    const nodes = [
      room("room", { parent }),
      door("meta", "working", "writing", { parent }),
      terminal("running", { parent }),
      terminal("exited", { parent }),
      ...kinds.map((kind) => agent(kind, kind, "writing", { parent })),
    ];

    const { rail } = await mountRail([
      ...Array.from({ length: depth }, (_, index) =>
        room(`home-${index}`, { parent: index === 0 ? null : `home-${index - 1}` })),
      ...nodes,
    ], [exitedTerminal("exited", 0)]);

    for (const node of nodes) {
      const row = within(screen.getByRole("tree")).getByText(node.name, { selector: ".name" })
        .closest<HTMLElement>("[role=treeitem]")!;

      for (const state of ["rest", "hover", "selected"] as const) {
        if (state === "hover") fireEvent.mouseEnter(row);

        if (state === "selected") {
          fireEvent.mouseLeave(row);
          rail.select(node.id);
        }

        const band = getComputedStyle(row);
        const line = getComputedStyle(row.querySelector(".line")!);
        // jsdom has no layout engine; resolve the inherited minimum to check the centering box against the painted band.
        const lineHeight = Number.parseFloat(line.minHeight === "inherit" ? band.minHeight : line.minHeight) || 0;
        const bandHeight = Number.parseFloat(band.minHeight);

        expect({ height: bandHeight, centerOffset: (bandHeight - lineHeight) / 2, alignment: line.alignItems,
          display: line.display, margin: line.marginTop }, `${node.name} ${state}`).toEqual({
          height: 28, centerOffset: 0, alignment: "center", display: "flex", margin: "0px",
        });
      }
    }
  });
});
