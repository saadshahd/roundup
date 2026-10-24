import { captureFocus } from "../keys/focus";
import { ReservedLine } from "../ink/ReservedLine";
import { copy } from "../copy";
import { createEffect, createSignal, createUniqueId, onCleanup, onMount } from "solid-js";
import { Portal } from "solid-js/web";
import { CrepeBuilder } from "@milkdown/crepe/builder";
import { blockEdit } from "@milkdown/crepe/feature/block-edit";
import type { BlockEditFeatureConfig } from "@milkdown/crepe/feature/block-edit";
import { linkTooltip } from "@milkdown/crepe/feature/link-tooltip";
import { linkPreviewTooltip } from "@milkdown/kit/component/link-tooltip";
import { listItem } from "@milkdown/crepe/feature/list-item";
import { placeholder } from "@milkdown/crepe/feature/placeholder";
import { topBar } from "@milkdown/crepe/feature/top-bar";
import { editorViewCtx, remarkStringifyOptionsCtx } from "@milkdown/kit/core";
import { $prose } from "@milkdown/kit/utils";
import { trailingConfig } from "@milkdown/kit/plugin/trailing";
import type { Node as DocumentNode } from "@milkdown/kit/prose/model";
import { Plugin, TextSelection } from "@milkdown/kit/prose/state";
import { getMarkdown, replaceAll } from "@milkdown/kit/utils";
import { sourceForDocument } from "./sourceForDocument";
import { visualBlocks } from "./visualBlocks";
import { visualKinds } from "./visualKinds";
import "@milkdown/crepe/theme/common/style.css";
import "@milkdown/crepe/theme/classic.css";

export type PadEditorHandle = {
  /** null pauses saving because the edited blocks cannot be mapped to authored source. */
  getText(): string | null;
  setText(text: string): void;
  focus(): void;
  snapshot(): { source: string; text: string; document: string; trailingCaret: boolean };
  /** Restores a native document without changing its source baseline or focus; false means invalid data. */
  restoreDocument(document: string, trailingCaret: boolean): boolean;
  /** Endpoints are ProseMirror document positions; restoration changes no focus. */
  selection(): { start: number; end: number; direction: "forward" | "backward" | "none" };
  restoreSelection(selection: { start: number; end: number; direction: "forward" | "backward" | "none" }): void;
  /** T27: brings the first heading whose text is `heading` (case, whitespace runs and leading `#` ignored, as P15 reads it) to the top of the view. */
  scrollToHeading(heading: string): void;
  /** The document equals the one last loaded, whatever Markdown it would serialize to. */
  unchanged(): boolean;
};

const topBarNames = new Map([
  ["bold", copy.bold],
  ["italic", copy.italic],
  ["strikethrough", copy.strikethrough],
  ["code", copy.inlineCode],
  ["bullet-list", copy.bulletList],
  ["ordered-list", copy.numberedList],
  ["task-list", copy.checklist],
  ["link", copy.link],
  ["code-block", copy.codeBlock],
  ["quote", copy.quote],
  ["hr", copy.horizontalRule],
]);

const normalHeading = (text: string) => text.replace(/^#+/, "").split(/\s+/).filter(Boolean).join(" ").toLowerCase();

/** U147: the Pad's one surface. `readonly` is ownership (P2), never a mode; `onChange` fires on every document change the user makes, `onText` follows with the settled Markdown. */
export default function PadRichEditor(props: {
  initialFocus?: ReturnType<typeof captureFocus>;
  text: string;
  readonly: boolean;
  onChange: () => void;
  onText: (text: string) => void;
  onBlur: (next: EventTarget | null) => void;
  onReady: (editor: PadEditorHandle) => void;
  onSelection?: () => void;
}) {
  const menuId = createUniqueId();
  let menuObserver: MutationObserver | undefined;
  let host: HTMLDivElement | undefined;
  let menuHost: HTMLDivElement | undefined;
  let crepe: CrepeBuilder | undefined;
  let created: ReturnType<CrepeBuilder["create"]> | undefined;
  const caretNodes = new WeakSet<DocumentNode>();
  let original = props.text;
  let applying = false;
  let closed = false;

  // The restored selection, kept until the user's first input: Chromium may still hand ProseMirror the caret it adopted at focus.
  let restored: { start: number; end: number; direction: "forward" | "backward" | "none" } | null = null;
  let restoreAgain = () => {};

  let ready = false;
  let topBarLabels: string[] = [];
  const [failure, setFailure] = createSignal<string | null>(null);

  onMount(() => {
    const focus = props.initialFocus ?? captureFocus();

    if (!host || !menuHost) throw new Error(copy.padEditorHostIsMissing);

    // SAFETY: Crepe recursively partials HTMLElement in its root type; the supported root is a real DOM element.
    const blockConfig = {
      blockHandle: { getOffset: () => 0 },
      slashMenu: { root: menuHost },
    } as BlockEditFeatureConfig;

    crepe = new CrepeBuilder({
      root: host,
      defaultValue: props.text,
    })
      .setReadonly(props.readonly)
      .addFeature(blockEdit, blockConfig)
      .addFeature(listItem)
      .addFeature(linkTooltip)
      .addFeature(placeholder)
      .addFeature(topBar, {
        buildTopBar: (builder) => {
          topBarLabels = builder.build().flatMap((group) =>
            group.items
              .filter((item) => item.key !== "heading-selector")
              .map((item) => {
                const label = topBarNames.get(item.key);

                if (!label) throw new Error(copy.padEditorToolbarActionHasNoName(item.key));

                return label;
              }),
          );
        },
        headingOptions: [
          { label: copy.text, level: null },
          { label: copy.headingOne, level: 1 },
          { label: copy.headingTwo, level: 2 },
          { label: copy.headingThree, level: 3 },
        ],
      });

    crepe.editor.config((ctx) => {
      // Hovering a link while selecting text must not open a floating preview; the fixed bar still edits links.
      ctx.update(linkPreviewTooltip.key, (options) => ({ ...options, props: { ...options.props, handleDOMEvents: {} } }));
      ctx.update(remarkStringifyOptionsCtx, (options) => ({ ...options, bullet: "-" as const }));
      ctx.update(trailingConfig.key, (options) => ({
        ...options,
        getNode: (state) => {
          const node = options.getNode(state);
          caretNodes.add(node);

          return node;
        },
      }));
    });

    crepe.editor.use(
      $prose(
        () =>
          new Plugin({
            view: () => ({
              update: (view, previous) => {
                if (!applying && !closed && !view.state.doc.eq(previous.doc)) {
                  restored = null;
                  props.onChange();
                } else if (!applying && !closed && restored && !view.state.selection.eq(previous.selection) && !view.composing) restoreAgain();
                else if (!applying && !closed && !view.state.selection.eq(previous.selection)) props.onSelection?.();
              },
            }),
          }),
      ),
    );

    crepe.editor.use($prose(() => visualBlocks(visualKinds(), () => props.readonly)));

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

      if (!field) throw new Error(copy.padEditorDidNotOpen);

      const buttons = host.querySelectorAll<HTMLButtonElement>(".milkdown-top-bar .top-bar-item");

      if (buttons.length !== topBarLabels.length) throw new Error(copy.padEditorToolbarChanged);

      buttons.forEach((button, index) => {
        const label = topBarLabels[index];

        if (!label) throw new Error(copy.padEditorToolbarLabelIsMissing);

        button.setAttribute("aria-label", label);
        button.title = label;
      });

      const heading = host.querySelector<HTMLButtonElement>(".milkdown-top-bar .top-bar-heading-button");

      if (heading) {
        heading.setAttribute("aria-label", copy.heading);
        heading.title = copy.heading;
      }

      field.setAttribute("role", "textbox");
      field.setAttribute("aria-label", copy.padBody);
      field.setAttribute("aria-multiline", "true");

      // Crepe owns navigation; mirror its rendered selection without intercepting keys.
      const updateMenu = () => {
        const menu = menuHost?.querySelector(".milkdown-slash-menu");
        const list = menu?.querySelector(".menu-groups");
        const shown = menu?.getAttribute("data-show") === "true";
        const items = list?.querySelectorAll<HTMLElement>("li[data-index]");
        let active: string | undefined;

        if (list) {
          list.id = menuId;
          list.setAttribute("role", "listbox");
          list.setAttribute("aria-label", copy.blocks);
          list.querySelectorAll("ul").forEach((group) => group.setAttribute("role", "presentation"));
        }

        items?.forEach((item) => {
          item.id = `${menuId}-${item.dataset.index}`;
          item.setAttribute("role", "option");
          const selected = item.classList.contains("hover");

          item.setAttribute("aria-selected", String(selected));

          if (selected) active = item.id;
        });

        if (shown && active) {
          field.setAttribute("aria-controls", menuId);
          field.setAttribute("aria-activedescendant", active);
        } else {
          field.removeAttribute("aria-controls");
          field.removeAttribute("aria-activedescendant");
        }

        host?.querySelectorAll(".milkdown-block-handle .operation-item")
          .forEach((item) => item.setAttribute("aria-hidden", "true"));
      };

      menuObserver = new MutationObserver(updateMenu);
      menuObserver.observe(menuHost!, {
        subtree: true, childList: true, attributes: true, attributeFilter: ["class", "data-show"],
      });
      menuObserver.observe(host, { subtree: true, childList: true });
      updateMenu();

      const document = () => crepe!.editor.action((ctx) => ctx.get(editorViewCtx).state.doc);
      let originalDocument = document();

      const getText = () => crepe!.editor.action((ctx) => sourceForDocument(original, originalDocument, document(), ctx, caretNodes));

      ready = true;
      crepe.setReadonly(props.readonly);

      // ProseMirror's own focus writes its selection to the DOM first, so Chromium has no caret of its own to adopt.
      const focusEditor = () => crepe!.editor.action((ctx) => ctx.get(editorViewCtx).focus());

      if (!props.readonly) focus(focusEditor);

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
        snapshot: () => ({
          source: original,
          text: crepe!.editor.action(getMarkdown()),
          document: JSON.stringify(document().toJSON()),
          trailingCaret: Boolean(document().lastChild && caretNodes.has(document().lastChild!)),
        }),
        restoreDocument: (json, trailingCaret) => crepe!.editor.action(ctx => {
          const view = ctx.get(editorViewCtx);
          let next: DocumentNode;

          try {
            const value: unknown = JSON.parse(json);
            next = view.state.schema.nodeFromJSON(value);
            next.check();

            if (next.type !== view.state.schema.topNodeType) return false;
          } catch {
            return false;
          }

          applying = true;

          try {
            view.dispatch(view.state.tr.replaceWith(0, view.state.doc.content.size, next.content));

            if (trailingCaret && view.state.doc.lastChild?.content.size === 0) caretNodes.add(view.state.doc.lastChild);
          } finally {
            applying = false;
          }

          return true;
        }),
        selection: () => crepe!.editor.action(ctx => {
          const { anchor, head, from, to } = ctx.get(editorViewCtx).state.selection;

          return { start: from, end: to, direction: anchor === head ? "none" : anchor < head ? "forward" : "backward" };
        }),
        restoreSelection: (selection) => crepe!.editor.action(ctx => {
          const view = ctx.get(editorViewCtx);

          const apply = () => {
            const start = Math.min(selection.start, view.state.doc.content.size);
            const end = Math.min(selection.end, view.state.doc.content.size);
            applying = true;

            try {
              view.dispatch(view.state.tr.setSelection(TextSelection.create(view.state.doc, selection.direction === "backward" ? end : start, selection.direction === "backward" ? start : end)));
            } finally {
              applying = false;
            }
          };

          const release = () => { restored = null; restoreAgain = () => {}; };

          restored = selection;
          restoreAgain = apply;

          view.dom.addEventListener("keydown", release, { once: true });
          view.dom.addEventListener("pointerdown", release, { once: true });

          apply();
        }),
        focus: focusEditor,
        unchanged: () => document().eq(originalDocument),
        scrollToHeading: (heading) => {
          const wanted = normalHeading(heading);

          const scroll = () => crepe?.editor.action(ctx => {
            const view = ctx.get(editorViewCtx);

            view.state.doc.descendants((node, pos) => {
              if (node.type.name !== "heading" || normalHeading(node.textContent) !== wanted) return true;

              const element = view.nodeDOM(pos);

              if (element instanceof HTMLElement) {
                // The sticky formatting toolbar covers the top of the scroll port; the heading stops below it.
                const bar = element.closest(".milkdown")?.querySelector<HTMLElement>(".milkdown-top-bar");

                element.style.scrollMarginTop = `${bar?.offsetHeight ?? 0}px`;
                element.scrollIntoView({ block: "start" });
              }

              return false;
            });
          });

          // One frame after the loaded text is in, the layout holds: aim once.
          requestAnimationFrame(() => { if (!closed) scroll(); });
        },
      });
    }).catch((error) => {
      if (!closed) setFailure(error instanceof Error ? error.message : String(error));
    });
  });

  createEffect(() => {
    const readonly = props.readonly;

    if (ready) crepe?.setReadonly(readonly);
  });

  onCleanup(() => {
    closed = true;
    menuObserver?.disconnect();

    // Destroying during create schedules a retry that can outlive the host's DOM environment.
    if (crepe) void created?.then(
      () => crepe!.destroy(),
      // Creation failures already reach the failure line; destroy failures must still escape.
      () => undefined,
    );
  });

  return (
    <>
      <Portal>
        <div class="pad-rich-editor pad-block-menu">
          <div class="milkdown" ref={(element) => (menuHost = element)} />
        </div>
      </Portal>
      <div
        class="pad-editor pad-rich-editor"
        classList={{ "pad-readonly": props.readonly }}
        onFocusOut={(event) => props.onBlur(event.relatedTarget)}>
        <ReservedLine message={failure()}><button data-button="quiet" type="button" onClick={() => setFailure(null)}>{copy.dismiss}</button></ReservedLine>
        <div ref={(element) => (host = element)} />
      </div>
    </>
  );
}
