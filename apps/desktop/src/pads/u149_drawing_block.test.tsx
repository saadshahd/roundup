import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createElement } from "react";
import { AGENT, openPad, openShelf, padOf } from "./padsFixture";
import { USER } from "../testing/nodes";
import type { CanvasApi, CanvasProps } from "./drawingBlock";
import { bundledRenderers, useRenderers } from "./visualKinds";

type Props = CanvasProps;

let props: Props | undefined;

let loads: unknown[] = [];

const fake: CanvasApi = {
  // The skeleton expands to full elements.
  convertToExcalidrawElements: (skeleton, options) => (loads.push(options), skeleton).map((element, index) => ({ id: String(element.id ?? `e${index}`), version: 1 })),
  Excalidraw: (next) => {
    props = next;

    return createElement("div", { "data-testid": "canvas", "data-readonly": String(next.viewModeEnabled) }, `${next.initialData.elements.length} elements`);
  },
};

// jsdom has no matchMedia; the canvas theme follows the colour scheme.
window.matchMedia ??= (query) => ({ ...new EventTarget(), matches: false, media: query, onchange: null, addListener: () => undefined, removeListener: () => undefined });

beforeEach(() => {
  useRenderers({ ...bundledRenderers, canvas: async () => fake });
});

afterEach(() => {
  cleanup();
  useRenderers(bundledRenderers);
  props = undefined;
  loads = [];
});

const current = () => {
  if (!props) throw new Error("the canvas did not mount");

  return props;
};

const scene = '{"type":"excalidraw","version":2,"elements":[{"type":"rectangle","id":"a","x":0,"y":0,"width":160,"height":80,"label":{"text":"plan"}},{"type":"text","id":"t","x":220,"y":28,"text":"ship"},{"type":"arrow","x":160,"y":40,"width":60,"height":0,"start":{"id":"a"},"end":{"id":"t"}}]}';

const block = `\`\`\`excalidraw\n${scene}\n\`\`\`\n`;

const writes = (calls: { method: string; params?: unknown }[]) => calls.filter((call) => call.method === "pad.write");

describe("u149 a Drawing renders and edits in the Pad", () => {
  it("u149_the_fixture_renders_inline_on_a_canvas", async () => {
    await openShelf([padOf("note", AGENT, `intro\n\n${block}`)]);
    await openPad("note");

    await screen.findByTestId("canvas");
    expect(screen.getByTestId("canvas").textContent).toBe("3 elements");
    expect(document.querySelector(".visual-drawing")).not.toBeNull();
  });

  it("u149_an_agent_owned_drawing_is_read_only_and_append_still_works", async () => {
    const { app } = await openShelf([padOf("note", AGENT, block)]);

    await openPad("note");
    expect((await screen.findByTestId("canvas")).dataset.readonly).toBe("true");

    const append = screen.getByLabelText("append");

    fireEvent.input(append, { target: { value: " more" } });
    fireEvent.keyDown(append, { key: "Enter" });

    await waitFor(() => expect(app.calls.filter((call) => call.method === "pad.append")).toHaveLength(1));
  });

  it("u149_a_user_owned_drawing_is_editable", async () => {
    await openShelf([padOf("note", USER, block)]);
    await openPad("note");

    expect((await screen.findByTestId("canvas")).dataset.readonly).toBe("false");
  });

  it("u149_an_unchanged_drawing_keeps_its_block_byte_for_byte_and_writes_nothing", async () => {
    const source = `${block}\ntail  \n`;
    const { app, connected, state } = await openShelf([padOf("note", USER, source)]);
    const field = await openPad("note");

    await screen.findByTestId("canvas");
    // The canvas reports the scene it loaded, as Excalidraw does on mount.
    current().onChange(current().initialData.elements);
    fireEvent.focusOut(field);
    connected.drawer.close();

    await new Promise((done) => setTimeout(done, 0));
    expect(writes(app.calls)).toHaveLength(0);
    expect(state.pads[0]?.text).toBe(source);
  });

  it("u149_a_moved_shape_writes_the_serialized_scene_once_and_nothing_else_changes", async () => {
    const { app, connected } = await openShelf([padOf("note", USER, `before\n\n${block}\nafter\n`)]);
    const field = await openPad("note");
    const host = (await screen.findByTestId("canvas")).closest(".visual-drawing")!;

    fireEvent.pointerDown(host);
    const [first, ...rest] = current().initialData.elements;

    current().onChange([{ ...first!, version: 2 }, ...rest]);
    fireEvent.pointerUp(window);
    fireEvent.focusOut(field);
    connected.drawer.close();

    await waitFor(() => expect(writes(app.calls)).toHaveLength(1));
    const text = String(Object.entries(writes(app.calls)[0]?.params ?? {}).find(([key]) => key === "text")?.[1]);
    const body = text.split("\n").find((line) => line.startsWith("{")) ?? "";

    expect(text.startsWith("before\n\n```excalidraw\n")).toBe(true);
    expect(text.endsWith("```\n\nafter\n")).toBe(true);
    expect(JSON.parse(body)).toMatchObject({ type: "excalidraw", version: 2, elements: [{ id: "a", version: 2 }, { id: "t" }, {}] });
  });

  it("u149_a_body_the_canvas_cannot_load_shows_its_text_and_one_error_and_writes_nothing", async () => {
    const source = "before\n\n```excalidraw\n{not json\n```\n\nafter\n";
    const { app, connected } = await openShelf([padOf("note", USER, source)]);
    const field = await openPad("note");

    await waitFor(() => expect(document.querySelectorAll(".visual-error")).toHaveLength(1));
    expect(document.querySelector(".visual-block pre")?.textContent).toBe("{not json");
    expect(field.textContent).toContain("after");

    fireEvent.focusOut(field);
    connected.drawer.close();
    await new Promise((done) => setTimeout(done, 0));
    expect(writes(app.calls)).toHaveLength(0);
  });

  it("u149_a_scene_without_elements_is_rejected_with_one_error", async () => {
    await openShelf([padOf("note", AGENT, '```excalidraw\n{"type":"other"}\n```\n')]);
    await openPad("note");

    await waitFor(() => expect(document.querySelectorAll(".visual-error")).toHaveLength(1));
    expect(document.querySelector(".visual-error")?.textContent).toBe("Drawing: not an Excalidraw scene");
  });

  it("u149_a_pad_changed_with_a_new_drawing_renders_it", async () => {
    const { app, state } = await openShelf([padOf("note", AGENT, "plain")]);

    await openPad("note");
    expect(document.querySelector(".visual-drawing")).toBeNull();

    state.pads[0]!.text = `plain\n\n${block}`;
    app.emit({ actor: AGENT, name: "pad.changed", data: { name: "note" } });

    await screen.findByTestId("canvas");
  });

  it("u149_unsafe_markup_in_a_scene_text_never_becomes_dom", async () => {
    await openShelf([padOf("note", AGENT, '```excalidraw\n{"type":"excalidraw","version":2,"elements":[{"type":"text","text":"<img src=x onerror=alert(1)>"}]}\n```\n')]);
    await openPad("note");

    await screen.findByTestId("canvas");
    expect(document.querySelector("[onerror], .visual-drawing img")).toBeNull();
  });

  it("u149_a_stored_scene_reloads_without_new_ids_so_bindings_hold", async () => {
    await openShelf([padOf("note", USER, block)]);
    await openPad("note");
    await screen.findByTestId("canvas");

    expect(loads).toEqual([{ regenerateIds: false }]);
  });

  it("u149_adding_a_shape_then_leaving_writes_once_with_only_the_block_changed", async () => {
    const { app, connected } = await openShelf([padOf("note", USER, `before\n\n${block}\nafter\n`)]);
    const field = await openPad("note");
    const host = (await screen.findByTestId("canvas")).closest(".visual-drawing")!;

    fireEvent.pointerDown(host);
    current().onChange([...current().initialData.elements, { id: "new", version: 1 }]);
    fireEvent.pointerUp(window);
    fireEvent.focusOut(field);
    connected.drawer.close();

    await waitFor(() => expect(writes(app.calls)).toHaveLength(1));
    const text = String(Object.entries(writes(app.calls)[0]?.params ?? {}).find(([key]) => key === "text")?.[1]);
    const body = text.split("\n").find((line) => line.startsWith("{")) ?? "";

    expect(text.startsWith("before\n\n```excalidraw\n")).toBe(true);
    expect(text.endsWith("```\n\nafter\n")).toBe(true);
    expect(JSON.parse(body).elements).toHaveLength(4);
  });

  it("u149_a_load_time_version_bump_is_not_an_edit", async () => {
    const source = `${block}`;
    const { app, connected } = await openShelf([padOf("note", USER, source)]);
    const field = await openPad("note");
    const host = (await screen.findByTestId("canvas")).closest(".visual-drawing")!;
    const [first, ...rest] = current().initialData.elements;

    current().onChange([{ ...first!, version: 5 }, ...rest]);
    fireEvent.pointerDown(host);
    fireEvent.pointerUp(window);
    fireEvent.focusOut(field);
    connected.drawer.close();

    await new Promise((done) => setTimeout(done, 0));
    expect(writes(app.calls)).toHaveLength(0);
  });

  it("u149_a_dirty_draft_shows_the_conflict_line_after_pad_changed", async () => {
    const { app, state } = await openShelf([padOf("note", USER, `plain\n\n${block}`)]);
    const field = await openPad("note");

    fireEvent.focus(field);
    fireEvent.input(field, { target: { value: "plain edited" } });
    state.pads = [padOf("note", USER, `plain\n\n${block}\nagent line\n`)];
    app.emit({ actor: AGENT, name: "pad.changed", data: { name: "note" } });

    await screen.findByText("changed by auth-refactor");
  });

  it("u149_an_unsafe_link_in_a_scene_makes_no_live_link", async () => {
    await openShelf([padOf("note", AGENT, '```excalidraw\n{"type":"excalidraw","version":2,"elements":[{"type":"rectangle","id":"r","link":"javascript:alert(1)"}]}\n```\n')]);
    await openPad("note");

    await screen.findByTestId("canvas");
    expect(document.querySelector('.visual-drawing a[href^="javascript"]')).toBeNull();
  });

  it("u149_invalid_json_leaves_the_rest_of_the_pad_editable", async () => {
    await openShelf([padOf("note", USER, "before\n\n```excalidraw\n{not json\n```\n\nafter\n")]);
    const field = await openPad("note");

    await waitFor(() => expect(document.querySelectorAll(".visual-error")).toHaveLength(1));
    expect(screen.getByRole("textbox", { name: "Pad body" }).getAttribute("contenteditable")).toBe("true");
    expect(field.readOnly).toBe(false);
  });

  it("u149_a_rejected_scene_leaves_the_rest_of_the_pad_editable", async () => {
    await openShelf([padOf("note", USER, 'before\n\n```excalidraw\n{"type":"other"}\n```\n')]);
    const field = await openPad("note");

    await waitFor(() => expect(document.querySelectorAll(".visual-error")).toHaveLength(1));
    expect(screen.getByRole("textbox", { name: "Pad body" }).getAttribute("contenteditable")).toBe("true");
    expect(field.readOnly).toBe(false);
  });
});
