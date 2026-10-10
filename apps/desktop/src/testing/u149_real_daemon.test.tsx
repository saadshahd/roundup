// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { createElement } from "react";
import { App } from "../App";
import { bundledRenderers, useRenderers } from "../pads/visualKinds";
import { fakeEmulators } from "../terminal/paneHarness";
import { createServedApp } from "./realSeam";
import { startRealDaemon } from "./realDaemonFixture";
import type { StartedDaemon } from "./realDaemonFixture";

let real: StartedDaemon;

beforeAll(async () => {
  real = await startRealDaemon({});
}, 120_000);

afterAll(async () => {
  await real?.stop();
});

afterEach(() => {
  cleanup();
  useRenderers(bundledRenderers);
});

window.matchMedia ??= (query) => ({ ...new EventTarget(), matches: false, media: query, onchange: null, addListener: () => undefined, removeListener: () => undefined });

// jsdom has no layout; ProseMirror asks a Range for its rectangles when a selection moves.
Range.prototype.getClientRects ??= () => document.body.getClientRects();

Range.prototype.getBoundingClientRect ??= () => new DOMRect();

const mermaid = "```mermaid\ngraph TD\n  A --> B\n```\n";

const drawing = '```excalidraw\n{"type":"excalidraw","version":2,"elements":[{"type":"rectangle","id":"a","x":0,"y":0,"width":160,"height":80}]}\n```\n';

describe("u148, u149 the open Drawer renders a block the Daemon stores", () => {
  it("u149_a_stored_drawing_renders_in_the_open_drawer_and_reads_back_byte_exact", async () => {
    useRenderers({
      mermaid: async () => ({ initialize: () => undefined, parse: async () => undefined, render: async () => ({ svg: "<svg></svg>" }) }),
      canvas: async () => ({
        Excalidraw: () => createElement("div", { "data-testid": "canvas" }),
        convertToExcalidrawElements: (skeleton) => skeleton.map((element, index) => ({ id: String(element.id ?? index), version: 1 })),
      }),
    });
    const app = createServedApp(real.base);

    await app.rpc("pad.create", { name: "sketch", text: `${mermaid}\n${drawing}` });
    render(() => <App app={app} reducedMotion={() => false} clock={Date.now} createEmulator={fakeEmulators().factory} />);
    fireEvent.click(await screen.findByText("sketch", {}, { timeout: 20_000 }));

    await screen.findByTestId("canvas", {}, { timeout: 20_000 });
    await waitFor(() => expect(document.querySelectorAll(".visual-block").length).toBe(2));
    expect((await app.rpc("pad.read", { name: "sketch" })).text).toBe(`${mermaid}\n${drawing}`);
  }, 60_000);
});
