import { cleanup } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { openPad, openShelf, padOf } from "./padsFixture";
import { USER } from "../testing/nodes";
import { bundledRenderers, useRenderers } from "./visualKinds";

afterEach(() => {
  cleanup();
  useRenderers(bundledRenderers);
});

describe("u149 the canvas loads only when a Drawing is present", () => {
  it("u149_a_pad_without_a_drawing_never_requests_the_module", async () => {
    let requested = 0;

    useRenderers({
      ...bundledRenderers,
      canvas: async () => {
        requested += 1;
        throw new Error("The canvas was requested");
      },
    });
    await openShelf([padOf("note", USER, "# Heading\n\n```json\n{}\n```\n")]);
    await openPad("note");

    await new Promise((done) => setTimeout(done, 20));
    expect(requested).toBe(0);
  });
});
