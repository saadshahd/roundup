import { Plugin, TextSelection } from "@milkdown/kit/prose/state";
import type { EditorState } from "@milkdown/kit/prose/state";
import type { Node as DocumentNode } from "@milkdown/kit/prose/model";
import { Decoration, DecorationSet } from "@milkdown/kit/prose/view";
import type { EditorView } from "@milkdown/kit/prose/view";

export type VisualContext = {
  text: string;
  readonly: boolean;
  /** Replaces this block's text, as the user's own edit of the document. */
  replace(text: string): void;
};

/** Fills `host` with the rendering and returns what releases it. */
export type VisualMount = (host: HTMLElement, context: VisualContext) => () => void;

export type VisualKind = {
  mount: VisualMount;
  /** Mermaid shows its text while the cursor is in it; a Drawing is edited only on its canvas. */
  textWhenFocused: boolean;
};

type Block = { node: DocumentNode; pos: number; kind: VisualKind };

const blocksOf = (doc: DocumentNode, kinds: ReadonlyMap<string, VisualKind>) => {
  const blocks: Block[] = [];

  doc.descendants((node, pos) => {
    if (node.type.name !== "code_block") return;
    const kind = kinds.get(String(node.attrs.language ?? "").trim().toLowerCase());

    if (kind) blocks.push({ node, pos, kind });
  });

  return blocks;
};

const focusedIn = (state: EditorState, { node, pos }: Block) =>
  state.selection.from > pos && state.selection.to < pos + node.nodeSize;

/**
 * U148, U149: a fenced block of a visual kind (`mermaid`, `excalidraw`) renders beside its own text. The document keeps the code block, so the stored Markdown is the block's text byte for byte; only an edit changes it.
 */
export const visualBlocks = (kinds: ReadonlyMap<string, VisualKind>, readonly: () => boolean) => {
  // What each block's widget was built from, and the text it last wrote itself: a write of its own keeps the widget, any other change builds a new one.
  const slots = new Map<number, { loaded: string; own: string }>();
  // The editor's own first focus puts the cursor in a leading block; only the user's key or pointer opens a block's text.
  let armed = false;

  const arm = () => {
    armed = true;

    return false;
  };

  return new Plugin({
    props: {
      handleDOMEvents: { keydown: arm, mousedown: arm },
      decorations(state) {
        const decorations: Decoration[] = [];
        const locked = readonly();

        blocksOf(state.doc, kinds).forEach((block, index) => {
          const text = block.node.textContent;
          const slot = slots.get(index) ?? { loaded: text, own: text };

          if (text !== slot.own) {
            slot.loaded = text;
            slot.own = text;
          }

          slots.set(index, slot);
          const open = block.kind.textWhenFocused && !locked && armed && focusedIn(state, block);

          decorations.push(Decoration.node(block.pos, block.pos + block.node.nodeSize, { class: open ? "visual-source is-open" : "visual-source" }));

          if (open) return;

          let release: (() => void) | undefined;

          decorations.push(
            Decoration.widget(
              block.pos + block.node.nodeSize,
              (view) => {
                const host = document.createElement("div");

                host.className = "visual-block";
                host.contentEditable = "false";
                release = block.kind.mount(host, {
                  text: slot.loaded,
                  readonly: locked,
                  replace: (next) => {
                    const target = blocksOf(view.state.doc, kinds)[index];

                    if (!target || target.node.textContent === next) return;
                    slot.own = next;
                    replaceText(view, target, next);
                  },
                });

                if (!locked && block.kind.textWhenFocused)
                  host.addEventListener("click", () => {
                    armed = true;
                    const target = blocksOf(view.state.doc, kinds)[index];

                    if (!target) return;
                    view.dispatch(view.state.tr.setSelection(TextSelection.create(view.state.doc, target.pos + 1)));
                    view.focus();
                  });

                return host;
              },
              {
                key: `${index}:${locked ? "r" : "w"}:${slot.loaded}`,
                side: 1,
                ignoreSelection: true,
                stopEvent: () => true,
                destroy: () => release?.(),
              },
            ),
          );
        });

        return DecorationSet.create(state.doc, decorations);
      },
    },
  });
};

const replaceText = (view: EditorView, { node, pos }: Block, next: string) => {
  const from = pos + 1;
  const to = pos + node.nodeSize - 1;
  const content = next === "" ? [] : [view.state.schema.text(next)];

  view.dispatch(view.state.tr.replaceWith(from, to, content));
};
