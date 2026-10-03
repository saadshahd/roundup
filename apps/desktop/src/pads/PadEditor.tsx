import { onCleanup, onMount } from "solid-js";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { tags } from "@lezer/highlight";

export type PadEditorHandle = {
  getText(): string;
  setText(text: string): void;
  focus(): void;
};

const highlight = HighlightStyle.define([
  { tag: tags.heading, fontWeight: "600" },
  { tag: tags.strong, fontWeight: "600" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.monospace, fontFamily: "var(--font-mono)" },
  { tag: [tags.link, tags.url], color: "var(--accent)" },
  { tag: [tags.processingInstruction, tags.meta], color: "var(--grey)" },
]);

export default function PadEditor(props: {
  text: string;
  onText: (text: string) => void;
  onBlur: (next: EventTarget | null) => void;
  onReady: (editor: PadEditorHandle) => void;
}) {
  let host: HTMLDivElement | undefined;
  let editor: EditorView | undefined;
  let applying = false;

  onMount(() => {
    if (!host) throw new Error("Pad Editor host is missing");

    editor = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: props.text,
        extensions: [
          markdown(),
          syntaxHighlighting(highlight),
          history(),
          keymap.of([...defaultKeymap, ...historyKeymap]),
          EditorView.lineWrapping,
          EditorView.contentAttributes.of({ "aria-label": "Editor" }),
          EditorView.updateListener.of((update) => {
            if (update.docChanged && !applying) props.onText(update.state.doc.toString());
          }),
          EditorView.domEventHandlers({ blur: (event) => props.onBlur(event.relatedTarget) }),
        ],
      }),
    });

    props.onReady({
      getText: () => editor!.state.doc.toString(),
      setText: (text) => {
        const current = editor!.state.doc.toString();

        if (text !== current) {
          applying = true;
          editor!.dispatch({ changes: { from: 0, to: current.length, insert: text } });
          applying = false;
        }
      },
      focus: () => editor!.focus(),
    });
    editor.focus();
  });

  onCleanup(() => editor?.destroy());

  return <div class="pad-editor" ref={(element) => (host = element)} />;
}
