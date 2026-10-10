import { cleanup } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { openPad, openShelf, padOf } from "./padsFixture";
import { USER } from "../testing/nodes";
import { bundledRenderers, useRenderers } from "./visualKinds";

afterEach(() => {
  cleanup();
  useRenderers(bundledRenderers);
});

describe("u148 Mermaid loads only when a block is present", () => {
  it("u148_a_pad_without_a_block_never_requests_the_module", async () => {
    let requested = 0;

    useRenderers({
      ...bundledRenderers,
      mermaid: async () => {
        requested += 1;
        throw new Error("Mermaid was requested");
      },
    });
    await openShelf([padOf("note", USER, "# Heading\n\n```js\nlet a = 1\n```\n")]);
    await openPad("note");

    await new Promise((done) => setTimeout(done, 20));
    expect(requested).toBe(0);
  });
});
