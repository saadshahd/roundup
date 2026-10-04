import { KindGlyph } from "../ink/KindGlyph";
import { createSignal, onCleanup, onMount, Show } from "solid-js";
import { CrepeBuilder } from "@milkdown/crepe/builder";
import { linkTooltip } from "@milkdown/crepe/feature/link-tooltip";
import { listItem } from "@milkdown/crepe/feature/list-item";
import { placeholder } from "@milkdown/crepe/feature/placeholder";
import { toolbar } from "@milkdown/crepe/feature/toolbar";
import { topBar } from "@milkdown/crepe/feature/top-bar";
import { editorViewCtx } from "@milkdown/kit/core";
import { replaceAll } from "@milkdown/kit/utils";
import type { PadEditorHandle } from "./PadEditor";
import { sourceForDocument } from "./sourceForDocument";
import "@milkdown/crepe/theme/common/style.css";
import "@milkdown/crepe/theme/classic.css";

const topBarNames = new Map([
  ["bold", "Bold"],
  ["italic", "Italic"],
  ["strikethrough", "Strikethrough"],
  ["code", "Inline code"],
  ["bullet-list", "Bullet list"],
  ["ordered-list", "Numbered list"],
  ["task-list", "Checklist"],
  ["link", "Link"],
  ["code-block", "Code block"],
  ["quote", "Quote"],
  ["hr", "Horizontal rule"],
]);

export default function PadRichEditor(props: {
  text: string;
  onText: (text: string) => void;
  onBlur: (next: EventTarget | null) => void;
  onReady: (editor: PadEditorHandle) => void;
}) {
  let host: HTMLDivElement | undefined;
  let crepe: CrepeBuilder | undefined;
  let created: ReturnType<CrepeBuilder["create"]> | undefined;
  let original = props.text;
  let applying = false;
  let closed = false;
  let topBarLabels: string[] = [];
  const [failure, setFailure] = createSignal<string | null>(null);

  onMount(() => {
    if (!host) throw new Error("Pad Editor host is missing");

    crepe = new CrepeBuilder({
      root: host,
      defaultValue: props.text,
    })
      .addFeature(listItem)
      .addFeature(linkTooltip)
      .addFeature(placeholder)
      .addFeature(toolbar)
      .addFeature(topBar, {
        buildTopBar: (builder) => {
          topBarLabels = builder.build().flatMap((group) =>
            group.items
              .filter((item) => item.key !== "heading-selector")
              .map((item) => {
                const label = topBarNames.get(item.key);

                if (!label) throw new Error(`Pad Editor toolbar action ${item.key} has no name`);

                return label;
              }),
          );
        },
        headingOptions: [
          { label: "Text", level: null },
          { label: "Heading 1", level: 1 },
          { label: "Heading 2", level: 2 },
          { label: "Heading 3", level: 3 },
        ],
      });

    crepe.on((listener) => {
      listener.markdownUpdated((_ctx, markdown) => {
        if (applying || closed) return;

        props.onText(markdown);
      });
    });

    created = crepe.create();
    void created.then(() => {
      if (closed || !host || !crepe) return;

      const field = host.querySelector<HTMLElement>(".ProseMirror");

      if (!field) throw new Error("Pad Editor did not open");

      const buttons = host.querySelectorAll<HTMLButtonElement>(".milkdown-top-bar .top-bar-item");

      if (buttons.length !== topBarLabels.length) throw new Error("Pad Editor toolbar changed");

      buttons.forEach((button, index) => {
        const label = topBarLabels[index];

        if (!label) throw new Error("Pad Editor toolbar label is missing");

        button.setAttribute("aria-label", label);
        button.title = label;
      });

      field.setAttribute("role", "textbox");
      field.setAttribute("aria-label", "Editor");
      field.setAttribute("aria-multiline", "true");

      const document = () => crepe!.editor.action((ctx) => ctx.get(editorViewCtx).state.doc);
      let originalDocument = document();
      const getText = () => sourceForDocument(original, originalDocument, document(), () => crepe!.getMarkdown());

      props.onReady({
        getText,
        setText: (text) => {
          if (text === getText()) {
            original = text;
            originalDocument = document();

            return;
          }

          applying = true;

          try {
            crepe!.editor.action(replaceAll(text));
            original = text;
            originalDocument = document();
          } finally {
            applying = false;
          }
        },
        focus: () => field.focus(),
      });
      field.focus();
    }).catch((error) => {
      if (!closed) setFailure(error instanceof Error ? error.message : String(error));
    });
  });

  onCleanup(() => {
    closed = true;

    // Destroying during create schedules a retry that can outlive the host's DOM environment.
    if (crepe) void created?.then(
      () => crepe!.destroy(),
      // Creation failures already reach the failure line; destroy failures must still escape.
      () => undefined,
    );
  });

  return (
    <div class="pad-editor pad-rich-editor" onFocusOut={(event) => props.onBlur(event.relatedTarget)}>
      <Show when={failure()}>
        {(message) => <p role="alert" class="pad-editor-failure"><KindGlyph kind="error" decorative /> {message()}</p>}
      </Show>
      <div ref={(element) => (host = element)} />
    </div>
  );
}
