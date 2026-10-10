import { createElement } from "react";
import type { ComponentType } from "react";
import { createRoot } from "react-dom/client";
import * as v from "valibot";
import type { VisualMount } from "./visualBlocks";

export type CanvasElement = { id: string; version: number; isDeleted?: boolean };

export type CanvasProps = {
  initialData: { elements: CanvasElement[]; scrollToContent: boolean };
  viewModeEnabled: boolean;
  theme: "light" | "dark";
  UIOptions: { canvasActions: Record<string, boolean> };
  onChange(elements: readonly CanvasElement[]): void;
};

/** The calls the Pad makes of Excalidraw. */
export type CanvasApi = {
  Excalidraw: ComponentType<CanvasProps>;
  /** Expands skeletons: a shape with a `label`, an arrow with `start` and `end` ids. Ids are kept, so a stored scene's bindings still point at their elements. */
  convertToExcalidrawElements(skeleton: Skeleton[], options: { regenerateIds: false }): CanvasElement[];
};

export type Skeleton = { type: string; id?: string | undefined };

const Scene = v.object({ type: v.literal("excalidraw"), elements: v.array(v.looseObject({ type: v.string(), id: v.optional(v.string()) })) });

const parse = (text: string) => {
  const scene = v.safeParse(Scene, JSON.parse(text));

  if (!scene.success) throw new Error("not an Excalidraw scene");

  return scene.output.elements;
};

const signature = (elements: readonly CanvasElement[]) => elements.map((element) => `${element.id}:${element.version}`).join(",");

/** U149: an Excalidraw scene on a canvas, editable unless the Pad is read-only. */
export const mountDrawing = (canvas: CanvasApi): VisualMount => (host, { text, readonly, replace }) => {
  let root: ReturnType<typeof createRoot> | undefined;
  let pending: string | undefined;
  let pointerDown = false;
  // Loading may touch versions; only the user's own gesture counts as an edit.
  let touched = false;

  const flush = () => {
    if (pending === undefined) return;
    const next = pending;

    pending = undefined;
    replace(next);
  };

  const up = () => {
    pointerDown = false;
    flush();
  };

  const down = () => {
    pointerDown = true;
    touched = true;
  };

  host.classList.add("visual-drawing");
  host.addEventListener("pointerdown", down);
  host.addEventListener("keydown", () => (touched = true));
  window.addEventListener("pointerup", up);

  try {
    const elements = canvas.convertToExcalidrawElements(parse(text), { regenerateIds: false });
    let last = signature(elements);

    root = createRoot(host);
    root.render(
      createElement(canvas.Excalidraw, {
        initialData: { elements, scrollToContent: true },
        viewModeEnabled: readonly,
        theme: matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light",
        UIOptions: { canvasActions: { loadScene: false, saveToActiveFile: false, export: false, saveAsImage: false } },
        onChange: (current) => {
          if (readonly) return;

          if (!touched) {
            last = signature(current);

            return;
          }

          if (signature(current) === last) return;
          last = signature(current);
          pending = JSON.stringify({ type: "excalidraw", version: 2, elements: current.filter((element) => !element.isDeleted) });

          if (!pointerDown) flush();
        },
      }),
    );
  } catch (error) {
    const source = document.createElement("pre");
    const line = document.createElement("p");

    source.textContent = text;
    line.className = "visual-error";
    line.setAttribute("role", "alert");
    line.textContent = `Drawing: ${error instanceof Error ? error.message : String(error)}`;
    host.replaceChildren(source, line);
  }

  return () => {
    pending = undefined;
    window.removeEventListener("pointerup", up);
    // Unmounting during React's own render pass throws; defer it.
    queueMicrotask(() => root?.unmount());
  };
};
