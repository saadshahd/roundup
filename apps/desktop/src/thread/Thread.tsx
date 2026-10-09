import { createEffect, createMemo, createSignal, For, onCleanup, Show } from "solid-js";
import type { Accessor, JSX } from "solid-js";
import { ErrorLine } from "../ink/ErrorLine";
import { Icon } from "../ink/Icon";
import { useConnectedProject } from "../state/connectedProject";
import { sizeText, sortFiles } from "./attachment";
import type { Attachment } from "./attachment";
import { createMessageFeed } from "./feed";
import { foldedLine, fullLine, isBetweenAgents } from "./lines";
import { quotedLines, quoteOf } from "./quote";
import type { Quote } from "./quote";
import "./styles.css";

type Preview = { attachment: Attachment; left: number; top: number };

const isPrintable = (press: KeyboardEvent): boolean => press.key.length === 1 && !press.ctrlKey && !press.metaKey;

const AttachmentChip = (props: { attachment: Attachment; onPreview: (preview: Preview | null) => void; onRemove?: () => void }) => (
  <span
    class="thread-attachment"
    data-attachment={props.attachment.id}
    onMouseEnter={(hover) => {
      const box = hover.currentTarget.getBoundingClientRect();

      props.onPreview({ attachment: props.attachment, left: box.left, top: box.top });
    }}
    onMouseLeave={() => props.onPreview(null)}
  >
    {props.attachment.name} <span class="light">{sizeText(props.attachment.size)}</span>
    <Show when={props.onRemove}>
      {(remove) => (
        <button type="button" class="word" aria-label={`remove ${props.attachment.name}`} onClick={remove()}>
          <Icon name="x" />
        </button>
      )}
    </Show>
  </span>
);

/**
 * The conversation with the selected Room's Door: the feed of Messages (U106), and the input (U105, U109, U110).
 * `selectionOf` reads the selected Terminal's selection through the emulator the Pane owns.
 */
const ThreadBody = (props: { door: Accessor<string>; selectionOf: (terminalId: string | null) => string }) => {
  const { app, rail, events, reducedMotion } = useConnectedProject();
  const feed = createMessageFeed(app, events);
  const [expanded, setExpanded] = createSignal(false);
  const [value, setValue] = createSignal("");
  const [quotes, setQuotes] = createSignal<readonly Quote[]>([]);
  const [attachments, setAttachments] = createSignal<readonly Attachment[]>([]);
  const [kept, setKept] = createSignal<readonly Attachment[]>([]);
  const [preview, setPreview] = createSignal<Preview | null>(null);
  const [failure, setFailure] = createSignal<string | null>(null);
  const [typing, setTyping] = createSignal(false);
  const [sending, setSending] = createSignal(false);
  const [root, setRoot] = createSignal<HTMLElement>();
  let feedElement: HTMLElement | undefined;
  let field: HTMLInputElement | undefined;
  let nextAttachment = 1;
  let fromFeed: { text: string; source: string } | null = null;
  let usedFromTerminal = "";

  const nodeIds = createMemo(() => new Set(rail.nodes.map((node) => node.id)));
  const nameOf = (id: string) => rail.nodes.find((node) => node.id === id)?.name ?? id;

  const lines = createMemo(() =>
    feed.messages().map((message) => {
      const from = rail.nameOf(message.from);
      const to = nameOf(message.to);
      const folded = isBetweenAgents(message, nodeIds()) && !expanded();

      return { id: message.id, status: message.status, folded, text: folded ? foldedLine(from, to, message) : fullLine(from, to, message) };
    }),
  );

  // U105: the periphery lives outside this region, so the dim is a mark on the window that the stylesheet reads.
  createEffect(() => {
    const window = root()?.closest<HTMLElement>(".window");

    if (!window) return;

    window.dataset.typing = String(typing());
    window.dataset.reducedMotion = String(reducedMotion());
  });

  onCleanup(() => {
    const window = root()?.closest<HTMLElement>(".window");

    if (window) {
      delete window.dataset.typing;
      delete window.dataset.reducedMotion;
    }
  });

  const within = (node: Node | null, ancestor: Element | undefined | null): boolean => ancestor != null && node !== null && ancestor.contains(node);

  // U110: focus moving to the input empties the document's selection, so the feed's selection is kept as it is made.
  const onSelectionChange = () => {
    const selection = document.getSelection();
    const text = selection?.toString() ?? "";
    const anchor = selection?.anchorNode ?? null;

    if (within(anchor, field)) {
      if (text !== "") fromFeed = null;
    } else if (within(anchor, feedElement)) {
      const message = (anchor instanceof Element ? anchor : anchor?.parentElement)?.closest<HTMLElement>("[data-message]");

      fromFeed = text === "" ? null : { text, source: message ? `message ${message.dataset.message}` : "thread" };
    } else if (text !== "") {
      fromFeed = null;
    }
  };

  const onEscape = (press: KeyboardEvent) => {
    if (press.key === "Escape") setPreview(null);
  };

  document.addEventListener("selectionchange", onSelectionChange);
  document.addEventListener("keydown", onEscape);
  onCleanup(() => {
    document.removeEventListener("selectionchange", onSelectionChange);
    document.removeEventListener("keydown", onEscape);
  });

  /** U110: the selection waiting in the feed or in the selected Terminal's output; each is used up by the Quote it makes. */
  const takeSelection = (): Quote | null => {
    if (fromFeed !== null) {
      const quote = quoteOf(fromFeed.text, fromFeed.source);

      fromFeed = null;

      return quote;
    }

    const node = rail.nodes.find((candidate) => candidate.id === rail.selected());
    const text = props.selectionOf(node?.terminal_id ?? null);

    if (text === "") {
      usedFromTerminal = "";

      return null;
    }

    if (text === usedFromTerminal) return null;

    usedFromTerminal = text;

    return quoteOf(text, node?.name ?? "");
  };

  const addFiles = (files: Iterable<File>) => {
    const { ok, refused } = sortFiles(files);

    if (refused.length > 0) setFailure(`${refused[0]} is not an image`);

    setAttachments((now) => [...now, ...ok.map((file) => ({ id: nextAttachment++, name: file.name, size: file.size, url: URL.createObjectURL(file) }))]);
  };

  const nothingToSend = () => value().trim() === "" && quotes().length === 0 && attachments().length === 0;

  const submit = async () => {
    if (sending()) return;

    const typed = value().trim();
    const body = [...quotes().map(quotedLines), typed].filter((part) => part !== "").join("\n");

    setTyping(false);

    if (body !== "") {
      setSending(true);

      try {
        await app.rpc("message.send", { to: props.door(), kind: "note", body, replyTo: null });
      } catch (error) {
        if (!(error instanceof Error)) throw error;

        setFailure(error.message);

        return;
      } finally {
        setSending(false);
      }
    }

    setFailure(null);
    setKept((now) => [...now, ...attachments()]);
    setAttachments([]);
    setQuotes([]);
    setValue("");
  };

  const onFieldKeyDown = (press: KeyboardEvent) => {
    if (press.key === "Enter") {
      press.preventDefault();
      void submit();

      return;
    }

    if (!isPrintable(press)) return;

    const quote = takeSelection();

    if (quote !== null) setQuotes((now) => [...now, quote]);
  };

  const onKeyDown: JSX.EventHandler<HTMLElement, KeyboardEvent> = (press) => {
    if (press.ctrlKey && !press.metaKey && !press.altKey && !press.shiftKey && press.code === "KeyO") {
      press.preventDefault();
      setExpanded((now) => !now);
    }
  };

  return (
    <div class="thread" ref={setRoot} onKeyDown={onKeyDown}>
      <div class="thread-feed" role="log" aria-label="thread" tabIndex={0} ref={(element) => (feedElement = element)}>
        <For each={lines()}>
          {(line) => (
            <p class="thread-line" data-message={line.id} data-status={line.status} data-folded={line.folded}>
              {line.text}
            </p>
          )}
        </For>
        <For each={kept()}>
          {(attachment) => (
            <p class="thread-line">
              <AttachmentChip attachment={attachment} onPreview={setPreview} />
            </p>
          )}
        </For>
      </div>
      <div class="thread-failure">
        <Show when={failure() ?? feed.failure()}>{(message) => <ErrorLine message={message()} />}</Show>
      </div>
      <div
        class="thread-composer"
        onDragOver={(drag) => drag.preventDefault()}
        onDrop={(drop) => {
          drop.preventDefault();
          addFiles(drop.dataTransfer?.files ?? []);
        }}
      >
        <For each={quotes()}>
          {(quote, index) => (
            <span class="thread-quote" data-quote title={quote.text}>
              <span class="light">{quote.source}</span> {quote.text}
              <button type="button" class="word" aria-label={`remove quote from ${quote.source}`} onClick={() => setQuotes((now) => now.filter((_, at) => at !== index()))}>
                <Icon name="x" />
              </button>
            </span>
          )}
        </For>
        <For each={attachments()}>
          {(attachment) => (
            <AttachmentChip
              attachment={attachment}
              onPreview={setPreview}
              onRemove={() => setAttachments((now) => now.filter((each) => each.id !== attachment.id))}
            />
          )}
        </For>
        <input
          ref={(element) => (field = element)}
          data-thread-input
          aria-label="message"
          value={value()}
          onInput={(input) => {
            setValue(input.currentTarget.value);
            setTyping(true);
          }}
          onKeyDown={onFieldKeyDown}
          onBlur={() => setTyping(false)}
          onPaste={(paste) => {
            const files = paste.clipboardData?.files;

            if (!files || files.length === 0) return;

            paste.preventDefault();
            addFiles(files);
          }}
        />
        <button
          type="button"
          class="word thread-send"
          aria-disabled={nothingToSend() || sending() ? true : undefined}
          onClick={() => {
            if (!nothingToSend() && !sending()) void submit();
          }}
        >
          send
        </button>
      </div>
      <Show when={preview()}>
        {(shown) => (
          <img class="thread-preview" alt={shown().attachment.name} src={shown().attachment.url} style={{ left: `${shown().left}px`, top: `${shown().top}px` }} />
        )}
      </Show>
    </div>
  );
};

/** The selected Room, or the nearest one above the selected row; `null` when the selection resolves to no Room. */
const useDoorId = (): Accessor<string | null> => {
  const { rail } = useConnectedProject();

  return createMemo(() => {
    let id = rail.selected();

    while (id !== null) {
      const found = rail.nodes.find((node) => node.id === id);

      if (found === undefined) return null;

      if (found.kind === "room") return found.id;

      id = found.parent;
    }

    return null;
  });
};

/** U145: the Thread exists only where there is a Room whose Door it talks to. */
export const Thread = (props: { selectionOf: (terminalId: string | null) => string }) => {
  const door = useDoorId();

  return <Show when={door()}>{(id) => <ThreadBody door={id} selectionOf={props.selectionOf} />}</Show>;
};
