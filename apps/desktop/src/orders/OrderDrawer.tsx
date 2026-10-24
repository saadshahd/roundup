import { captureFocus, queueFocus } from "../keys/focus";
import { writeText } from "../app/clipboard";
import { ReservedLine } from "../ink/ReservedLine";
import * as v from "valibot";
import { RpcError, INTERNAL_CODE } from "../app/seam";
import { Tip } from "../ui/Tip";
import { IconButton } from "../ui/IconButton";
import { copy } from "../copy";
import type { Order } from "@contracts/agent/Order";
import { For, Match, Show, Switch, createEffect, createMemo, createResource, createSignal, createUniqueId, on, onCleanup } from "solid-js";
import { Icon } from "../ink/Icon";
import { ContextBar } from "./ContextBar";
import { Markdown } from "../plain/Markdown";
import "./styles.css";
import { useConnectedProject } from "../state/connectedProject";
import { WorkProgress } from "./WorkProgress";

type SpawnRead = { id: string; prompt: string | null; error: string | null };

type WorkOrder = Extract<Order, { kind: "work" }>;

const formSchema = v.object({
  title: v.string(), ask: v.string(), done: v.string(), field: v.picklist(["title", "ask", "done"]),
  baseTitle: v.nullable(v.string()), baseAsk: v.string(), baseDone: v.nullable(v.string()), limits: v.array(v.string()),
});

type OrderInput = {
  id: string; inline?: boolean;
  /** Loaded Project and Rail facts authorize recovery without a prompt read. */
  recovery?: { project: string; revision: string };
};

/** O17: the session id is muted text that copies itself on click and says so for a moment. */
const SessionId = (props: { id: string }) => {
  const { app } = useConnectedProject();
  const [note, setNote] = createSignal<string | null>(null);
  const [failure, setFailure] = createSignal<string | null>(null);
  const idText = createUniqueId();
  let timer: ReturnType<typeof setTimeout> | undefined;
  onCleanup(() => clearTimeout(timer));

  const copyId = async () => {
    clearTimeout(timer);

    try {
      await writeText(app, props.id);
      setFailure(null);
      setNote(copy.copied);
    } catch (error) {
      setNote(copy.copyFailed);
      setFailure(copy.copyDraftFailure(error instanceof Error ? error.message : String(error)));
    }

    clearTimeout(timer);
    timer = setTimeout(() => setNote(null), 1500);
  };

  return <>
    <button type="button" class="overview-id" aria-label={copy.copySessionId} aria-describedby={idText} onClick={() => void copyId()}>{note() ?? props.id}</button>
    <span id={idText} class="overview-id-note">{props.id}</span>
    <ReservedLine message={failure()} status={note()}><Show when={failure()}><IconButton aria-label={copy.clearName(copy.error.toLowerCase())} onClick={() => { setFailure(null); setNote(null); }}><Icon name="x" /></IconButton></Show></ReservedLine>
  </>;
};

/** O6: the whole order of one Agent or Door. What it shows follows the Rail, so a changed order reaches it. */
export const OrderDrawer = (props: OrderInput) =>
  <Show when={props.id} keyed>{(id) => <OrderContent {...props} id={id} />}</Show>;

const OrderContent = (props: OrderInput) => {
  const { rail, app, drafts, project } = useConnectedProject();
  const node = () => rail.nodes.find((each) => each.id === props.id);
  const order = () => node()?.work ?? null;
  const finished = () => node()?.status?.kind === "done";

  const retained = props.recovery ? drafts.list().find(draft => draft.revision === props.recovery?.revision
    && draft.destination.surface === "orders" && draft.destination.object === props.id
    && draft.destination.recipient === props.id) : undefined;

  if (props.recovery && (props.recovery.project !== project.path || rail.failure() !== null || !node()
    || !retained || !v.safeParse(formSchema, retained.value).success)) return null;

  const [editing, setEditing] = createSignal(props.recovery !== undefined);
  const [failure, setFailure] = createSignal<string | null>(null);
  const [promptDismissed, setPromptDismissed] = createSignal(false);
  createEffect(() => {
    if (!node()) {
      setFailure(null);
      setPromptDismissed(true);
    }
  });

  /** O10, U83: read on opening the order or selecting a closed Agent; a failed read keeps its text. */
  const [read] = createResource(() => props.recovery ? false : props.id, async (id): Promise<SpawnRead> => {
    try {
      const answer = await app.rpc("agent.spawnPrompt", { id });

      return { id, prompt: answer.prompt, error: null };
    } catch (thrown) {
      return { id, prompt: null, error: thrown instanceof Error ? thrown.message : copy.thePromptCouldNotBeRead };
    }
  });

  const spawn = () => read()?.id === props.id ? read() : undefined;
  const promptFailure = () => !promptDismissed() && node() ? spawn()?.error : null;
  const dismiss = () => { setFailure(null); setPromptDismissed(true); };

  let editButton: HTMLButtonElement | undefined;

  const close = (focus = captureFocus()) => {
    setEditing(false);
    setFailure(null);
    queueMicrotask(() => focus(() => editButton?.focus()));
  };

  const destination = () => ({ surface: "orders", object: props.id, recipient: props.id });
  const retentionFailure = () => drafts.problem() ? copy.draftRetention(project.name, copy.draftSurface(destination().surface), drafts.problem()!.cause) : null;

  createEffect(() => { if (editing()) onCleanup(drafts.editor(destination())); });

  const records = () => drafts.list().filter((draft) => draft.destination.surface === "orders"
    && draft.destination.object === props.id && draft.destination.recipient === props.id);

  const current = () => records().filter((draft) => draft.status === "draft").at(-1) ?? records().at(-1);
  const [selected, selectRevision] = createSignal<string | null>(retained?.revision ?? null);
  const selectedDraft = () => records().find((draft) => draft.revision === selected());

  const initial = createMemo(() => {
    const each = order();

    return each?.kind === "work" ? {
      title: each.title ?? "", ask: each.ask, done: each.done ?? "", field: "title",
      baseTitle: each.title ?? null, baseAsk: each.ask, baseDone: each.done ?? null, limits: [...each.limits],
    } : null;
  });

  const formValue = () => v.safeParse(formSchema, selectedDraft()?.value ?? initial());

  const base = (): WorkOrder | null => {
    const parsed = formValue();

    if (!parsed.success) return null;
    const value = parsed.output;

    const original: WorkOrder = { kind: "work", ask: value.baseAsk, limits: value.limits };

    if (value.baseTitle !== null) original.title = value.baseTitle;

    if (value.baseDone !== null) original.done = value.baseDone;

    return original;
  };

  const conflict = () => {
    const original = base();
    const latest = order();

    return selectedDraft() !== undefined && original !== null && (latest?.kind !== "work" || original.title !== latest.title
      || original.ask !== latest.ask || original.done !== latest.done
      || JSON.stringify(original.limits) !== JSON.stringify(latest.limits));
  };

  let form: HTMLFormElement | undefined;
  let disposed = false;
  let saveSequence = 0;
  let shownSequence = 0;
  onCleanup(() => { disposed = true; });

  const begin = () => {
    const draft = props.recovery ? selectedDraft() : current();
    const each = order();

    if (!draft && each?.kind !== "work") return;
    selectRevision(draft?.revision ?? null);
    setEditing(true);
  };

  const capture = (field: HTMLTextAreaElement) => {
    const parsed = formValue();

    if (!parsed.success || !form || selectedDraft()?.status === "unknown") return;
    const fields = new FormData(form);

    const edited = { ...parsed.output,
      title: String(fields.get("title") ?? ""), ask: String(fields.get("ask") ?? ""),
      done: String(fields.get("done") ?? ""), field: field.name,
    };

    const selection = { start: field.selectionStart, end: field.selectionEnd, direction: field.selectionDirection };
    const draft = selectedDraft();

    if (draft && edited.title === parsed.output.title && edited.ask === parsed.output.ask
      && edited.done === parsed.output.done && edited.field === parsed.output.field
      && selection.start === draft.selection?.start && selection.end === draft.selection?.end
      && selection.direction === draft.selection?.direction) return;
    const next = drafts.edit(destination(), edited, selection, "ordinary", props.recovery ? selected() : undefined);

    selectRevision(next.draft.revision);
  };

  const saveBlocked = () => (props.recovery !== undefined && (!node() || !selectedDraft())) || conflict() || (selectedDraft() !== undefined
    && !["draft", "failed"].includes(selectedDraft()!.status));

  const recovery = () => selectedDraft() !== undefined && (conflict() || drafts.problem() || ["failed", "unknown"].includes(selectedDraft()?.status ?? ""));

  /** Capture the revision and original recipient before crossing the RPC boundary. */
  const save = async () => {
    const focus = captureFocus();
    const draft = selectedDraft();
    const parsed = formValue();
    const captured = base();

    if (saveBlocked()) return;

    if (!draft) {
      close();

      return;
    }

    if (!parsed.success || !captured) return;
    const snapshot = drafts.submit(draft.revision);

    if (!snapshot) return;
    const sequence = ++saveSequence;
    const fields = parsed.output;
    const next: WorkOrder = { kind: "work", ask: fields.ask.trim(), limits: captured.limits };

    if (fields.title.trim()) next.title = fields.title.trim();

    if (fields.done.trim()) next.done = fields.done.trim();

    try {
      await app.rpc("agent.setOrder", {
        id: snapshot.draft.destination.object, order: next, captured: { order: captured },
      });
      drafts.settle(draft.revision, "acknowledged");

      if (!disposed) setPromptDismissed(true);

      // A late reply settles its own revision without replacing newer save feedback.
      if (!disposed && sequence >= shownSequence) {
        shownSequence = sequence;
        setFailure(null);
      }

      if (!disposed && selected() === draft.revision) close(focus);
    } catch (thrown) {
      drafts.settle(draft.revision, thrown instanceof RpcError && thrown.code !== INTERNAL_CODE ? "failed" : "unknown");

      if (!disposed && node() && sequence >= shownSequence) {
        shownSequence = sequence;
        setFailure(thrown instanceof Error && thrown.message !== "" ? thrown.message : copy.theOrderCouldNotBeSaved);
      }
    }
  };

  const resizeField = (field: HTMLTextAreaElement) => {
    field.style.height = "auto";
    field.style.height = `${field.scrollHeight + field.offsetHeight - field.clientHeight}px`;
  };

  const sizeField = (field: HTMLTextAreaElement) => {
    queueMicrotask(() => resizeField(field));

    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => resizeField(field));
    observer.observe(field);
    onCleanup(() => observer.disconnect());
  };

  const editor = () => (
    <form
      class="order-edit"
      ref={form}
      onSubmit={(submit) => {
        submit.preventDefault();
        void save();
      }}
      onKeyDown={(key) => {
        const single = key.target instanceof HTMLTextAreaElement && key.target.name !== "ask";

        if (key.key === "Enter" && single && !key.shiftKey) {
          key.preventDefault();
          key.currentTarget.requestSubmit();
        }

        if (key.key === "Escape") {
          key.preventDefault();
          key.stopPropagation();
          close();
        }
      }}
    >
      <For each={[{ name: "title", label: copy.title }, { name: "ask", label: copy.ask }, { name: "done", label: copy.doneWhen }] as const}>{(item) =>
        <label class="order-field">
          <span class="order-field-label">{item.label}</span>
          <textarea name={item.name} readOnly={selectedDraft()?.status === "unknown"} rows={item.name === "ask" ? 3 : 1} ref={(field) => {
            const parsed = formValue();

            if (parsed.success) field.value = String(parsed.output[item.name]);
            createEffect(on(initial, (latest) => {
              if (selectedDraft()) return;
              field.value = latest?.[item.name] ?? "";
              queueMicrotask(() => resizeField(field));
            }));
            const draft = selectedDraft();

            if (parsed.success && parsed.output.field === item.name) {
              if (draft?.selection) field.setSelectionRange(draft.selection.start, draft.selection.end, draft.selection.direction);
              queueFocus(() => { if (field.isConnected) field.focus(); });
            }

            sizeField(field);
          }} onInput={(event) => { resizeField(event.currentTarget); capture(event.currentTarget); }} onSelect={(event) => {
            const draft = selectedDraft();

            if (document.activeElement === event.currentTarget && draft?.status === "draft") capture(event.currentTarget);
          }} />
        </label>
      }</For>
      <ReservedLine message={failure() ?? (!formValue().success ? copy.orderRestoreFailed : conflict() ? copy.orderChanged : retentionFailure()) ?? (!props.inline && order()?.kind === "work" ? promptFailure() : null)} status={selectedDraft() && selectedDraft()!.status !== "draft" ? copy.draftStatus[selectedDraft()!.status] : null}>
        <Show when={failure() || (!props.inline && promptFailure())}><IconButton aria-label={copy.clearName(copy.error.toLowerCase())} onClick={dismiss}><Icon name="x" /></IconButton></Show>
        <Show when={drafts.problem()}><button data-button="quiet" type="button" onClick={() => drafts.retry()}>{copy.storageRetry}</button></Show>
      </ReservedLine>
      <div class="order-edit-actions order-edit-recovery">
        <Show when={recovery()}>
          <For each={["title", "ask", "done"]}>{(name) => <button data-button="quiet" type="button" onClick={() => void writeText(app, String(selectedDraft()?.value[name] ?? "")).then(() => setFailure(null)).catch((error) => setFailure(copy.copyDraftFailure(error instanceof Error ? error.message : String(error))))}>{copy.copyField(name)}</button>}</For>
          <button data-button="quiet" type="button" onClick={() => {
            const draft = selectedDraft();

            if (draft) drafts.discard(draft.revision);
            close();
          }}>{copy.discardRevision}</button>
        </Show>
      </div>
      <div class="order-edit-actions">
        <Tip disabled={!saveBlocked()} text={conflict() ? copy.orderChanged : selectedDraft() ? copy.draftStatus[selectedDraft()!.status] : undefined}>{(trigger) =>
          <button {...trigger()} data-button="primary" type="submit" aria-disabled={saveBlocked()}>{copy.save}</button>
        }</Tip>
        <button data-button="quiet" type="button" onClick={() => close()}>{copy.cancel}</button>
      </div>
    </form>
  );

  const shown = (each: WorkOrder) => (
    <>
      <div class="order-head">
        <Markdown class="order-title" body={each.title ?? each.ask} />
        <IconButton ref={editButton} aria-label={copy.editOrder} onClick={begin}>
          <Icon name="edit" />
        </IconButton>
      </div>
      <Show when={each.title}>
        <Markdown class="order-ask" body={each.ask} />
      </Show>
      <Show when={each.done}>
        {(done) => (
          <p class="order-done-line" data-done={finished()}>
            <span class="order-done-mark" role="img" aria-label={finished() ? copy.done : copy.notDone}>
              <Icon name={finished() ? "check" : "circle"} />
            </span>
            <span class="order-done">{done()}</span>
          </p>
        )}
      </Show>
    </>
  );

  return (
    <section
      class={props.inline ? "order-overview" : "order-drawer"}
      aria-label={copy.fullOrderOf(node()?.name ?? props.id)}
    >
      <Show when={props.inline ? node()?.overview : undefined}>
        {(overview) => <header class="overview-session">
          <p class="light">{overview().program}</p>
          <Show when={overview().name}>{(name) => <h2>{name()}</h2>}</Show>
          <Show when={overview().id} keyed>{id => <SessionId id={id} />}</Show>
          <ContextBar overview={overview()} />
          <div class="overview-card">
            <Show when={overview().card?.intent}>{(text) => <section><h3>{copy.cardIntent}</h3><Markdown body={text()} /></section>}</Show>
            <Show when={overview().card?.shape}>{(text) => <section><h3>{copy.cardShape}</h3><Markdown body={text()} /></section>}</Show>
            <Show when={overview().card?.facts?.length}><section><h3>{copy.cardFacts}</h3><ul><For each={overview().card?.facts}>{(fact) => <li><Markdown body={fact} /></li>}</For></ul></section></Show>
            <Show when={overview().card?.outcome}>{(text) => <section><h3>{copy.cardOutcome}</h3><Markdown body={text()} /></section>}</Show>
          </div>
        </header>}
      </Show>
      <Show when={editing()} fallback={<Switch>
        <Match when={order()} keyed>
          {(each) =>
            each.kind === "work" ? (
              <>
                {shown(each)}
                <Show when={props.inline && each.limits.length > 0}><section><h3>{copy.orderLimits}</h3><ul><For each={each.limits}>{(limit) => <li><Markdown body={limit} /></li>}</For></ul></section></Show>

              </>
            ) : (
              <Markdown class="order-ask" body={each.question} />
            )
          }
        </Match>
      </Switch>}>{editor()}</Show>
      <Show when={props.recovery === undefined}><Show when={props.id} keyed>{(id) => <WorkProgress id={id} />}</Show></Show>
      <ReservedLine message={!editing() ? retentionFailure() : null}>
        <button data-button="quiet" type="button" onClick={() => drafts.retry()}>{copy.storageRetry}</button>
      </ReservedLine>
      <ReservedLine class="order-spawn-error" hidden={props.inline || editing()} message={!props.inline && order()?.kind === "work" ? promptFailure() : null}><IconButton aria-label={copy.clearName(copy.error.toLowerCase())} onClick={dismiss}><Icon name="x" /></IconButton></ReservedLine>
    </section>
  );
};
