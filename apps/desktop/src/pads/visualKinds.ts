import { createElement } from "react";
import type { CanvasApi } from "./drawingBlock";
import { mountDrawing } from "./drawingBlock";
import type { MermaidApi } from "./mermaidBlock";
import { mountMermaid } from "./mermaidBlock";
import type { VisualKind, VisualMount } from "./visualBlocks";

/** Where each renderer comes from. A renderer's module is requested when the first block of its kind mounts, never for a Pad without one. */
export type Renderers = {
  mermaid(): Promise<MermaidApi>;
  canvas(): Promise<CanvasApi>;
};

export const bundledRenderers: Renderers = {
  mermaid: async () => {
    const { default: mermaid } = await import("mermaid");

    return {
      initialize: (config) => mermaid.initialize(config),
      parse: async (text) => {
        await mermaid.parse(text);
      },
      render: (id, text) => mermaid.render(id, text),
    };
  },
  canvas: async () => {
    const [module] = await Promise.all([import("@excalidraw/excalidraw"), import("@excalidraw/excalidraw/index.css")]);

    return {
      // SAFETY: CanvasProps lists the props this Pad passes, each a subset of what Excalidraw accepts and reports.
      Excalidraw: (props) => createElement(module.Excalidraw, props as never),
      // SAFETY: a skeleton is Excalidraw's own input shape, and its output elements all carry `id` and `version`.
      convertToExcalidrawElements: (skeleton, options) => module.convertToExcalidrawElements(skeleton as never, options),
      // SAFETY: stored elements are Excalidraw's own output, and restoring them keeps `id` and `version`.
      restoreElements: (elements) => module.restoreElements(elements as never, null) as never,
    };
  },
};

const lazy = <Api>(load: () => Promise<Api>, mount: (api: Api) => VisualMount): VisualMount => (host, context) => {
  let release: (() => void) | undefined;
  let live = true;

  void load().then((api) => {
    if (live) release = mount(api)(host, context);
  });

  return () => {
    live = false;
    release?.();
  };
};

let renderers = bundledRenderers;

/** Swaps where the renderers come from; a test passes its own. */
export const useRenderers = (next: Renderers) => {
  renderers = next;
};

export const visualKinds = () =>
  new Map<string, VisualKind>([
    ["mermaid", { textWhenFocused: true, mount: lazy(renderers.mermaid, mountMermaid) }],
    ["excalidraw", { textWhenFocused: false, insert: { command: "/drawing", language: "excalidraw", body: '{"type":"excalidraw","version":2,"elements":[]}' }, mount: lazy(renderers.canvas, mountDrawing) }],
  ]);
