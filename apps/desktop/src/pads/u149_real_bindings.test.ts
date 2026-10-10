import * as v from "valibot";
import { describe, expect, it } from "vitest";

const skeleton = [
  { type: "rectangle", id: "a", x: 0, y: 0, width: 160, height: 80, label: { text: "plan" } },
  { type: "text", id: "t", x: 220, y: 28, text: "ship" },
  { type: "arrow", x: 160, y: 40, width: 60, height: 0, start: { id: "a" }, end: { id: "t" } },
];

const Binding = v.nullish(v.object({ elementId: v.string() }));

const Stored = v.array(v.looseObject({ type: v.string(), startBinding: Binding, endBinding: Binding }));

describe("u149 a stored Drawing keeps its bindings through the real module", () => {
  it("u149_a_stored_arrow_keeps_both_bindings_after_a_reload", async () => {
    // jsdom has no 2d context; Excalidraw probes it for `filter` when it loads.
    HTMLCanvasElement.prototype.getContext = () => Object.assign(Object.create(null), { filter: "none", font: "", measureText: (text: string) => ({ width: text.length * 8, actualBoundingBoxAscent: 10, actualBoundingBoxDescent: 2 }) });
    // Nor fonts: text sizing registers font faces that nothing here draws.
    Object.assign(globalThis, { FontFace: class { load = async () => this; } });
    Object.assign(document, { fonts: { add: () => undefined, has: () => true, load: async () => [], ready: Promise.resolve() } });
    const canvas = await import("@excalidraw/excalidraw");
    // SAFETY: the literal above is a skeleton, Excalidraw's own input shape.
    const first = canvas.convertToExcalidrawElements(skeleton as never, { regenerateIds: false });
    const stored = v.parse(Stored, JSON.parse(JSON.stringify(first)));
    const arrow = v.parse(Stored, canvas.restoreElements(JSON.parse(JSON.stringify(stored)), null)).find((element) => element.type === "arrow");

    expect(arrow?.startBinding?.elementId).toBe("a");
    expect(arrow?.endBinding?.elementId).toBe("t");
  }, 60000);
});
