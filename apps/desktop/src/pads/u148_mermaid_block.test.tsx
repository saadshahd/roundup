import { cleanup, fireEvent, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { AGENT, openPad, openShelf, padOf } from "./padsFixture";
import { USER } from "../testing/nodes";
import type { MermaidApi } from "./mermaidBlock";
import { bundledRenderers, useRenderers } from "./visualKinds";

const rendered: string[] = [];

let config: Parameters<MermaidApi["initialize"]>[0] | undefined;

const fake: MermaidApi = {
  initialize: (next) => {
    config = next;
  },
  parse: async (text) => {
    if (text.includes("bad")) throw new Error("Parse error on line 1\n  code frame");
  },
  render: async (_id, text) => {
    rendered.push(text);

    return { svg: `<svg data-diagram="${text.length}"><text>drawn</text></svg>` };
  },
};

beforeEach(() => {
  useRenderers({ ...bundledRenderers, mermaid: async () => fake });
});

afterEach(() => {
  cleanup();
  useRenderers(bundledRenderers);
  rendered.length = 0;
});

const block = "```mermaid\ngraph TD\nA-->B\n```\n";

const writes = (calls: { method: string; params?: unknown }[]) => calls.filter((call) => call.method === "pad.write");

describe("u148 a Mermaid block renders in the Pad", () => {
  it("u148_a_valid_block_renders_svg_in_token_colours_with_strict_security", async () => {
    await openShelf([padOf("note", AGENT, `intro\n\n${block}`)]);
    await openPad("note");

    await waitFor(() => expect(document.querySelector(".visual-mermaid svg")).not.toBeNull());
    expect(rendered).toEqual(["graph TD\nA-->B"]);
    expect(config?.securityLevel).toBe("strict");
    expect(config?.themeVariables).toMatchObject({ fontFamily: expect.any(String), primaryTextColor: expect.any(String) });
  });

  it("u148_a_script_label_does_not_run", async () => {
    await openShelf([padOf("note", AGENT, "```mermaid\ngraph TD\nA[<script>window.hit=1</script>]\n```\n")]);
    await openPad("note");

    await waitFor(() => expect(document.querySelector(".visual-mermaid svg")).not.toBeNull());
    expect("hit" in window).toBe(false);
    expect(document.querySelector(".ProseMirror script")).toBeNull();
  });

  it("u148_invalid_text_shows_the_source_and_exactly_one_error_line", async () => {
    await openShelf([padOf("note", AGENT, "before\n\n```mermaid\nbad\n```\n\nafter")]);
    const field = await openPad("note");

    await waitFor(() => expect(document.querySelectorAll(".visual-error")).toHaveLength(1));
    expect(document.querySelector(".visual-error")?.textContent).toBe("Mermaid: Parse error on line 1");
    expect(document.querySelector(".visual-block pre")?.textContent).toBe("bad");
    expect(field.textContent).toContain("before");
    expect(field.textContent).toContain("after");
  });

  it("u148_an_unchanged_block_keeps_the_source_byte_for_byte", async () => {
    const source = `${block}\ntrailing  \n`;
    const { app, connected, state } = await openShelf([padOf("note", USER, source)]);
    const field = await openPad("note");

    await waitFor(() => expect(document.querySelector(".visual-mermaid svg")).not.toBeNull());
    fireEvent.click(document.querySelector(".visual-block")!);
    fireEvent.focusOut(field);
    connected.drawer.close();

    await new Promise((done) => setTimeout(done, 0));
    expect(writes(app.calls)).toHaveLength(0);
    expect(state.pads[0]?.text).toBe(source);
  });

  it("u148_a_focused_block_shows_its_text_and_a_changed_block_writes_once", async () => {
    const { app } = await openShelf([padOf("note", USER, block)]);
    const field = await openPad("note");

    await waitFor(() => expect(document.querySelector(".visual-mermaid svg")).not.toBeNull());
    fireEvent.click(document.querySelector(".visual-block")!);

    await waitFor(() => expect(document.querySelector(".visual-source.is-open")).not.toBeNull());
    expect(document.querySelector(".visual-block")).toBeNull();

    fireEvent.input(field, { target: { value: "```mermaid\ngraph LR\nA-->C\n```" } });
    fireEvent.focusOut(field);

    await waitFor(() => expect(writes(app.calls)).toHaveLength(1));
    expect(writes(app.calls)[0]?.params).toEqual({ name: "note", text: "```mermaid\ngraph LR\nA-->C\n```\n" });
  });

  it("u148_a_pad_changed_with_a_new_block_renders_it", async () => {
    const { app, state } = await openShelf([padOf("note", AGENT, "plain")]);

    await openPad("note");
    expect(document.querySelector(".visual-block")).toBeNull();

    state.pads[0]!.text = "plain\n\n```mermaid\ngraph TD\nA-->Z\n```\n";
    app.emit({ actor: AGENT, name: "pad.changed", data: { name: "note" } });

    await waitFor(() => expect(document.querySelector(".visual-mermaid svg")).not.toBeNull());
    expect(rendered).toEqual(["graph TD\nA-->Z"]);
  });
});
