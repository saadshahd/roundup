import { captureFocus } from "../keys/focus";
import { queueFocus } from "../keys/focus";
import { writeText } from "../app/clipboard";
import { copy } from "../copy";
import * as v from "valibot";
import type { Todo } from "@contracts/todo/Todo";
import type { Change } from "@contracts/todo/Change";
import { useConnectedProject } from "../state/connectedProject";
import { doesClearTodoFailure, outcomeOf } from "./failureOf";
import { ReservedLine } from "../ink/ReservedLine";
import { batch, createEffect, createSignal, For, on, onCleanup, Show } from "solid-js";

export const formSchema = v.strictObject({ text: v.string(), base: v.string(), home: v.optional(v.nullable(v.string()), null), expected_change: v.nullable(v.number()), title: v.string(), body: v.string(), blockers: v.array(v.string()), done: v.boolean() });

export const creationSchema = v.strictObject({ title: v.string(), home: v.nullable(v.string()) });

export const blockerSchema = v.strictObject({
  title: v.string(), body: v.string(), done: v.boolean(), blockers: v.array(v.string()), baseBlockers: v.array(v.string()), expected_change: v.nullable(v.number()),
  home: v.optional(v.nullable(v.string()), null), originalTitle: v.string(), originalBody: v.string(), originalDone: v.boolean(), originalBlockers: v.array(v.string()), originalChange: v.nullable(v.number()),
  action: v.picklist(["add", "remove"]), blocker: v.number(),
});

// Shared recovery also retains older title/text/choice snapshots for inspection, without an editable base.
const snapshotSchema = v.strictObject({ title: v.string(), body: v.optional(v.string()), choices: v.optional(v.array(v.string())) });

export const todoDraftSchema = v.union([creationSchema, formSchema, blockerSchema, snapshotSchema]);

type Editing = { form: v.InferOutput<typeof formSchema>; text: string; width: number; recovered: boolean; selection: { start: number; end: number; direction: "forward" | "backward" | "none" } | null };

/** Surface actions share the Project authority; selecting/copying an older revision never changes the editor. */
export const TodoDraftRecovery = (props: { object: string; active?: boolean; inDrawer?: boolean; saved?: () => number; onDiscard?: (revision: string) => void }) => {
  const { app, drafts, project, drawer, events } = useConnectedProject();
  const [failure, setFailure] = createSignal<string | null>(null);
  createEffect(on(props.saved ?? (() => 0), () => setFailure(null), { defer: true }));
  createEffect(on(() => props.object, () => setFailure(null), { defer: true }));
  onCleanup(events.subscribe(event => {
    if (doesClearTodoFailure(event, Number(props.object))) setFailure(null);
  }));
  const surface = copy.draftSurface("todos");
  const records = () => drafts.recoverable().filter(draft => draft.destination.surface === "todos" && draft.destination.object === props.object);

  const problem = () => {
    if (!props.active && records().length === 0) return null;

    const found = drafts.problem();

    return found?.destination && found.destination.surface !== "todos" ? null : found;
  };

  const label = (draft: ReturnType<typeof drafts.list>[number]) => copy.draftIdentity(
    copy.draftField(draft.destination.object === "new" ? "title" : draft.destination.recipient?.startsWith("blockers:") ? "blockers" : draft.destination.recipient ?? "title"),
    String(draft.value.text ?? draft.value.title ?? "").slice(0, 48),
  );

  const copyDraft = async (text: string) => {
    try { await writeText(app, text); setFailure(null); }
    catch (error) { if (!(error instanceof Error)) throw error; setFailure(copy.copyDraftFailure(error.message)); }
  };

  return <Show when={!drawer.content() || props.inDrawer}><section class="todo-draft-recovery" aria-label={copy.draftIdentity(surface, props.object)} onMouseDown={event => {
    if (event.target instanceof Element && event.target.closest("button")) event.preventDefault();
  }}>
    <ReservedLine overlay message={failure() ?? (problem() ? copy.draftRetention(project.name, copy.draftSurface(surface), problem()!.cause) : null)} status={records().length > 0 ? label(records()[records().length - 1]!) : null}>
    <Show when={failure()}><button data-dismiss-failure onMouseDown={event => event.preventDefault()} data-button="quiet" class="word" type="button" onClick={() => setFailure(null)}>{copy.dismissFailure}</button></Show>
    <Show when={problem()}><button data-button="quiet" class="word" type="button" onClick={() => drafts.retry()}>{copy.storageRetry}</button></Show>
    <For each={records().map(draft => draft.revision)}>{revision => {
      const draft = () => records().find(record => record.revision === revision);

      return <Show when={draft()}>{record => <div class="todo-draft-revision">
        <span id={`todo-context-${revision}`} class="todo-draft-label" hidden>{label(record())}</span>
        <span id={`todo-revision-${revision}`} data-state={record().status}>{copy.draftStatus[record().status]}</span>
        <div class="todo-draft-actions">
        <button data-button="quiet" class="word" type="button" aria-describedby={`todo-revision-${revision} todo-context-${revision}`} onClick={() => {
          const text = v.safeParse(v.string(), record().value.text ?? record().value.title);
          void copyDraft(!record().destination.recipient?.startsWith("blockers:") && text.success ? text.output : JSON.stringify(record().value));
        }}>{copy.copyField(surface)}</button>{" "}
        <button data-button="quiet" class="word" type="button" aria-describedby={`todo-revision-${revision} todo-context-${revision}`} onClick={() => { props.onDiscard?.(revision); drafts.discard(revision); }}>{copy.discardRevision}</button>
        </div>
      </div>}</Show>;
    }}</For>
    </ReservedLine>
  </section></Show>;
};

/**
 * Double-click edits; leaving the field commits once, and only when the text differs from what the field started
 * with, so another Actor's change made meanwhile is never written back. A failed commit reopens the field with
 * the typed text.
 */
export const EditableText = (props: {
  todo: Todo;
  captured: () => Change | undefined;
  /** A validated retained revision in this original Todo and field. */
  revision?: string | undefined;
  multiline: boolean;
  name: "title" | "body";
  saved?: () => number;
  onSaved?: () => void;
  onFailure?: (message: string | null, dismiss: () => void) => void;
  /** Resolves only after acknowledgement; the RPC error preserves refusal versus unknown acceptance. */
  commit: (text: string, expectedChange: number) => Promise<void>;
}) => {
  const connected = useConnectedProject();
  const draftDestination = { surface: "todos", object: String(props.todo.id), recipient: props.name };
  const records = () => connected.drafts.list().filter(draft => draft.destination.surface === "todos" && draft.destination.object === String(props.todo.id) && draft.destination.recipient === props.name);
  const [failure, setFailure] = createSignal<string | null>(null);
  const [retentionFailure, setRetentionFailure] = createSignal<string | null>(null);
  createEffect(on(props.saved ?? (() => 0), () => setFailure(null), { defer: true }));
  onCleanup(connected.events.subscribe(event => {
    if (doesClearTodoFailure(event, props.todo.id)) setFailure(null);
  }));
  const content = connected.drawer.content();
  let alive = true;
  onCleanup(() => { alive = false; });
  const [editing, setEditing] = createSignal<Editing | null>(null);

  const [activeRevision, setActiveRevision] = createSignal<string | null>(null);

  createEffect(() => {
    const revision = activeRevision();

    if (editing() !== null && revision !== null && !records().some(record => record.revision === revision)) setEditing(null);
  });

  createEffect(() => {
    if (editing() && connected.drawer.content() === content) onCleanup(connected.drafts.editor(draftDestination));
  });
  createEffect(() => props.onFailure?.(failure(), () => setFailure(null)));

  let shown: HTMLElement | undefined;

  const startWith = (width = shown?.getBoundingClientRect().width ?? 0) => {
    captureFocus(true);

    const saved = props.revision === undefined
      ? records().findLast(draft => draft.status === "draft" || draft.status === "failed")
      : records().find(draft => draft.revision === props.revision);

    const parsed = v.safeParse(formSchema, saved?.value);

    if (saved && !parsed.success) {
      setRetentionFailure(copy.draftRetention(connected.project.name, copy.draftSurface("todos"), "invalid"));

      return;
    }

    setRetentionFailure(null);
    const change = props.captured();
    const values = change?.after ?? props.todo;

    const form = parsed.success ? parsed.output : { text: values[props.name], base: values[props.name], expected_change: change?.seq ?? null,
      home: props.todo.home, title: values.title, body: values.body, blockers: values.blockers.map(String), done: values.done };

    setActiveRevision(saved?.revision ?? null);
    setEditing({ form, text: form.text, width, recovered: parsed.success, selection: saved?.selection ?? null });
  };

  if (props.revision !== undefined) startWith(0);

  const fit = (element: HTMLTextAreaElement) => {
    element.style.height = "auto";
    element.style.height = `${element.scrollHeight}px`;
  };

  let save: (() => Promise<void>) | undefined;
  let key: ((event: KeyboardEvent) => void) | undefined;

  const field = (started: Editing) => (element: HTMLTextAreaElement) => {
    element.value = started.text;
    const focus = captureFocus();
    // Fitted again once attached: `scrollHeight` is 0 before then.
    queueMicrotask(() => {
      fit(element);

      if (!alive) return;
      focus(() => element.focus({ preventScroll: true }));

      if (started.selection) element.setSelectionRange(started.selection.start, started.selection.end, started.selection.direction);
    });

    const retain = (selectionChanged = false) => batch(() => {
      const value = { ...started.form, text: element.value };
      const selection = { start: element.selectionStart, end: element.selectionEnd, direction: element.selectionDirection };
      const latest = records().find(record => record.revision === activeRevision());

      if (latest && JSON.stringify(v.parse(formSchema, latest.value)) === JSON.stringify(value) && (props.revision !== undefined || latest.status === "failed" || JSON.stringify(latest.selection) === JSON.stringify(selection))) {
        setActiveRevision(latest.revision);

        if ((props.revision === undefined || selectionChanged) && JSON.stringify(latest.selection) !== JSON.stringify(selection)) connected.drafts.setSelection(latest.revision, selection);

        return { draft: latest, problem: connected.drafts.problem() };
      }

      const retained = connected.drafts.edit(draftDestination, value, selection, "ordinary", activeRevision());

      setActiveRevision(retained.draft.revision);

      return retained;
    });

    // Delayed selection events from an editor that closed must not resurrect a discarded revision.
    let left = false;
    onCleanup(() => { left = true; });
    element.addEventListener("select", () => { if (!left && element.isConnected && (props.revision === undefined || document.activeElement === element) && (element.value !== started.form.base || started.recovered)) retain(true); });
    element.addEventListener("input", () => {
      if (left) return;

      if (!props.multiline && element.value.includes("\n")) element.value = element.value.replace(/\r?\n/g, " ");

      fit(element);
      retain();
    });

    const submit = async () => {
      if (left) return;
      const retryFocus = captureFocus();
      left = true;

      if (element.value === started.form.base && !started.recovered) {
        setEditing(null);

        return;
      }

      const draft = retain().draft;

      if (started.form.expected_change === null) {
        setFailure(copy.originalUnverified);
        left = false;

        return;
      }

      if (!connected.drafts.submit(draft.revision)) {
        setFailure(copy.draftStatus[draft.status]);
        left = false;

        return;
      }

      setEditing(null);

      try {
        await props.commit(element.value, started.form.expected_change);
        connected.drafts.settle(draft.revision, "acknowledged");

        if (alive) { setFailure(null); props.onSaved?.(); }
      } catch (error) {
        if (!(error instanceof Error)) throw error;
        connected.drafts.settle(draft.revision, outcomeOf(error));

        if (alive) {
          setFailure(error.message);

          if (outcomeOf(error) === "failed" && connected.drawer.content() === content && editing() === null && !records().some(record => record.status === "draft")
            && (document.activeElement === null || document.activeElement === document.body)) retryFocus(() => startWith(started.width));
        }
      }
    };

    key = event => {
      if (event.key === "Escape") {
        event.preventDefault(); event.stopPropagation();

        if (element.value !== started.form.base || started.recovered) retain();
        left = true; setEditing(null);
        queueFocus(() => shown?.focus());
      } else if (event.key === "Enter" && (!props.multiline || event.metaKey || event.ctrlKey)) {
        event.preventDefault(); void submit();
      }
    };

    element.addEventListener("blur", event => {
      if (left) return;

      if (element.value !== started.form.base || started.recovered) retain();
      const target = event.relatedTarget;
      queueMicrotask(() => {
        if (!alive || connected.drawer.content() !== content || started.recovered
          || (target instanceof Element && (target.closest(".close, .todo-draft-recovery, [data-dismiss-failure]") || !target.closest(".drawer")))) return;
        void submit();
      });
    });
    // Explicit Save authorizes restored work; restoration alone never resumes blur submission.
    save = submit;

  };

  return (
    <div class="todo-editable">
    <ReservedLine overlay message={retentionFailure() || (!props.onFailure && failure())}><Show when={!retentionFailure()}><button data-dismiss-failure onMouseDown={event => event.preventDefault()} data-button="quiet" class="word" type="button" onClick={() => setFailure(null)}>{copy.dismissFailure}</button></Show></ReservedLine>
    <Show
      when={editing()}
      fallback={
        <Show
          when={props.name === "body" && props.todo.body === ""}
          fallback={
            <p
              ref={(element) => { shown = element; }}
              tabIndex={0}
              onDblClick={() => startWith()}
              onKeyDown={(key) => {
                if (key.key === "Enter") startWith();
              }}
              style={{ "white-space": "pre-wrap", "overflow-wrap": "anywhere", "min-height": "1.4em", "flex-shrink": props.multiline ? "0" : "1" }}
            >
              <Show when={props.todo[props.name] !== ""} fallback={<span class="light">{copy.noField(props.name)}</span>}>
                {props.todo[props.name]}
              </Show>
            </p>
          }
        >
          {/* U3732: an empty body offers its own edit, where the body will read. */}
          <button ref={(element) => { shown = element; }} data-button="quiet" type="button" class="word todo-add-notes" onClick={() => startWith(0)}>{copy.addNotes}</button>
        </Show>
      }
      keyed
    >
      {(started) => (<>
        <ReservedLine message={(props.captured() && props.captured()?.seq !== started.form.expected_change) || (props.revision !== undefined && props.todo[props.name] !== started.form.base) ? copy.changedTodo(props.todo.title) : null} />
        <textarea
          ref={field(started)}
          aria-label={props.name === "title" ? copy.title : copy.body}
          rows={1}
          onKeyDown={event => key?.(event)}
          style={{
            display: "block",
            font: "inherit",
            "letter-spacing": "inherit",
            "line-height": "inherit",
            color: "inherit",
            "text-align": "inherit",
            "white-space": "pre-wrap",
            "overflow-wrap": "anywhere",
            padding: "0",
            margin: "0",
            border: "0",
            "box-sizing": "content-box",
            width: started.width > 0 ? `${started.width}px` : "100%",
            "flex-shrink": "0",
            "min-height": "1.4em",
            resize: "none",
            overflow: "hidden",
            background: "var(--hover)",
            "caret-color": "var(--accent)",
          }}
        />
      </>)}
    </Show>
    <Show when={editing()}><button data-button="quiet" class="word" type="button" onMouseDown={event => event.preventDefault()} onClick={() => void save?.()}>{copy.save}</button></Show>
    </div>
  );
};
