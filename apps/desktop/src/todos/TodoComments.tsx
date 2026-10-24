import { captureFocus } from "../keys/focus";
import { createEffect, createSignal, For, on, onCleanup, Show } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import type { Comment } from "@contracts/todo/Comment";
import { copy } from "../copy";
import { ReservedLine } from "../ink/ReservedLine";
import { KindGlyph } from "../ink/KindGlyph";
import { Collapsible } from "../ui/Collapsible";
import { useConnectedProject } from "../state/connectedProject";
import { doesClearTodoFailure, failureOf } from "./failureOf";
import { rowButtonOf, triage } from "./triage";

// Rows are keyed by id so fold state, focus and fade survive an arrival.
const byId = { key: "id" };

/** Unsent text per Todo, kept when the Drawer closes so Esc loses no draft. */
const drafts = new Map<number, string>();

type Part = { text: string } | { summary: string; details: string };

const isStatus = (part: Part): part is Extract<Part, { summary: string }> => "summary" in part;

const STATUS = "::status ";

/** U3775: an Agent writes `::status <summary>` and indents its detail lines by two spaces; a status line with no details stays plain text. */
const partsOf = (body: string, marked: boolean): Part[] => {
  const parts: Part[] = [];
  const plain: string[] = [];

  const flush = () => { if (plain.length > 0) parts.push({ text: plain.splice(0).join("\n") }); };

  const lines = body.split("\n");

  for (let at = 0; at < lines.length; at++) {
    const line = lines[at]!;

    if (!marked || !line.startsWith(STATUS)) { plain.push(line); continue; }

    const details: string[] = [];

    while (lines[at + 1]?.startsWith("  ")) details.push(lines[++at]!.slice(2));

    if (details.length === 0) { plain.push(line.slice(STATUS.length)); continue; }

    flush();
    parts.push({ summary: line.slice(STATUS.length), details: details.join("\n") });
  }

  flush();

  return parts;
};

/** The thread, oldest first, with a field to add to it. Opening reads it and marks it read once; a comment that arrives while it is open is read the same way. */
export const TodoComments = (props: { id: number; onSent?: ((focus: ReturnType<typeof captureFocus>) => void) | undefined; saved?: () => number; onSaved?: () => void }) => {
  const { app, events, rail, drawer } = useConnectedProject();
  // Keyed by id so a refresh keeps each comment's row, fold state and focus (U3776).
  const [thread, setThread] = createStore<Comment[]>([]);
  const [failure, setFailure] = createSignal<string | null>(null);
  const [sendFailure, setSendFailure] = createSignal<string | null>(null);
  createEffect(on(props.saved ?? (() => 0), () => setSendFailure(null), { defer: true }));
  const [text, setTextNow] = createSignal(drafts.get(props.id) ?? "");

  const setText = (value: string) => {
    setTextNow(value);

    if (value === "") drafts.delete(props.id);
    else drafts.set(props.id, value);
  };

  const [sending, setSending] = createSignal(false);
  // U3776: the comments shown once the Drawer has read them; later arrivals fade in and are announced once.
  const [fresh, setFresh] = createSignal<ReadonlySet<number>>(new Set<number>());
  const [announcement, setAnnouncement] = createSignal("");
  let shown: Set<number> | null = null;
  let latest = 0;
  let alive = true;
  let field: HTMLTextAreaElement | undefined;

  // The rendered text, never the `::status` marker or indent; it stays until the next arrival replaces it.
  const spoken = (comment: Comment) => partsOf(comment.body, comment.author.kind === "agent").map((part) => "summary" in part ? `${part.summary}\n${part.details}` : part.text).join("\n");

  // Only the newest fetch may write or mark, so overlapping answers neither reorder the thread nor mark twice.
  const refresh = async () => {
    const mine = ++latest;
    const id = props.id;

    const message = await failureOf(async () => {
      const comments = await app.rpc("todo.comments", { id });

      if (!alive || mine !== latest || id !== props.id) return;
      const arrived = shown === null ? [] : comments.filter((comment) => !shown!.has(comment.id));
      shown = new Set(comments.map((comment) => comment.id));
      setFresh((before) => new Set<number>([...before, ...arrived.map((comment) => comment.id)]));

      if (arrived.length > 0) setAnnouncement(arrived.map((comment) => `${author(comment)}: ${spoken(comment)}`).join("\n"));
      setThread(reconcile(comments, byId));

      if (comments.some((comment) => !comment.read)) await app.rpc("todo.markCommentsRead", { id });
    });

    if (alive && mine === latest && id === props.id) setFailure(message);
  };

  createEffect(on(() => props.id, () => { setThread(reconcile([], byId)); setFresh(new Set<number>()); setAnnouncement(""); shown = null; setFailure(null); setSendFailure(null); setTextNow(drafts.get(props.id) ?? ""); void refresh(); }));
  onCleanup(events.subscribe((event) => {
    if (doesClearTodoFailure(event, props.id)) setSendFailure(null);

    if (event.name === "todo.commented" && event.data.id === props.id) void refresh(); }));
  onCleanup(() => { alive = false; latest++; });

  // U3753: `c` on a triage row asks for this Todo's field.
  createEffect(() => {
    if (triage.commentFor() === props.id && field) {
      triage.focusComment(() => field!.focus());
      triage.askComment(null);
    }
  });

  const author = (comment: Comment) => comment.author.kind === "agent" ? rail.nodes.find((node) => node.id === comment.author.id)?.name ?? comment.author.id : copy.me;
  const glyphOf = (comment: Comment) => comment.author.kind === "agent" ? rail.nodes.find((node) => node.id === comment.author.id) : undefined;

  const send = async () => {
    const focus = captureFocus();
    const body = text();

    if (body.trim() === "" || sending()) return;
    const id = props.id;
    setSending(true);
    const message = await failureOf(async () => { await app.rpc("todo.comment", { id, body }); });
    setSending(false);

    if (!alive || id !== props.id) return;
    setSendFailure(message);

    // A rejected call keeps the typed text; the thread catches up through `todo.commented`.
    if (message === null) { if (text() === body) setText(""); props.onSaved?.(); await refresh(); props.onSent?.(focus); }
  };

  return <section class="todo-comments" aria-label={copy.commentsHeading(thread.length)}>
    <h3>{copy.commentsHeading(thread.length)}</h3>
    <ReservedLine message={failure()}><button data-button="quiet" class="word" type="button" onClick={() => void refresh()}>{copy.commentRetry}</button><button data-dismiss-failure onMouseDown={event => event.preventDefault()} data-button="quiet" class="word" type="button" onClick={() => setFailure(null)}>{copy.dismissFailure}</button></ReservedLine>
    <For each={thread}>{(comment) => <div class="todo-comment" data-comment={comment.id} data-fresh={fresh().has(comment.id) ? "" : undefined}>
      <p class="todo-comment-head"><Show when={glyphOf(comment)} fallback={<Show when={comment.author.kind === "agent"}><span class="todo-comment-glyph-gap" aria-hidden="true" /></Show>}>{(node) => <KindGlyph kind={node().status?.kind ?? "idle"} />}</Show><span class="todo-comment-author">{author(comment)}</span> <time datetime={new Date(comment.created_at).toISOString()}>{copy.commentTime(comment.created_at)}</time>
        <Show when={comment.author.kind === "agent"}><button data-button="quiet" class="word" type="button" onClick={() => field?.focus()}>{copy.reply}</button></Show></p>
      <div class="todo-comment-parts"><For each={partsOf(comment.body, comment.author.kind === "agent")}>{(part) => isStatus(part)
        ? <Collapsible.Root class="todo-status" lazyMount unmountOnExit>
          <Collapsible.Trigger data-button="quiet" class="word todo-status-summary">{part.summary}</Collapsible.Trigger>
          <Collapsible.Content class="todo-status-details">{part.details}</Collapsible.Content>
        </Collapsible.Root>
        : <p class="todo-comment-body">{part.text}</p>}</For></div>
    </div>}</For>
    <ReservedLine class="todo-comments-live" status={announcement()} aria-live="polite" aria-atomic="true" />
    <textarea class="todo-comment-field" rows={2} aria-label={copy.commentField} placeholder={copy.commentPlaceholder} value={text()}
      ref={(element) => { field = element; }}
      onInput={(event) => setText(event.currentTarget.value)}
      onKeyDown={(event) => {
        if (event.key === "Escape" && triage.on()) { event.preventDefault(); drawer.close("key"); rowButtonOf(props.id)?.focus(); }

        if (event.key === "Enter" && !event.shiftKey && !event.isComposing) { event.preventDefault(); void send(); }
      }} />
    <ReservedLine message={sendFailure()}><button data-dismiss-failure onMouseDown={event => event.preventDefault()} data-button="quiet" class="word" type="button" onClick={() => setSendFailure(null)}>{copy.dismissFailure}</button></ReservedLine>
    <p class="todo-comments-send"><button data-button="quiet" class="word" type="button" aria-disabled={text().trim() === "" || sending() ? true : undefined} onClick={() => void send()}>{copy.comment}</button></p>
  </section>;
};
