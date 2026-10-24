import { captureFocus } from "../keys/focus";
import { clipboardFailure, writeText } from "../app/clipboard";
import { createCutTitle } from "../ui/cutTitle";
import { copy } from "../copy";
import { createEffect, createSignal, For, on, onCleanup, Show } from "solid-js";
import type { FileComparison } from "@contracts/agent/FileComparison";
import { ReservedLine } from "../ink/ReservedLine";
import { Popover } from "../ui/Popover";
import { useConnectedProject } from "../state/connectedProject";
import { reply } from "./notes";
import { NoteForm } from "./NoteForm";
import type { NoteFormDraft } from "./notes";
import type { Note, Notes } from "./notes";

const olderComparison = (current: FileComparison | undefined, original: FileComparison) => current && current.newPath === original.newPath && (current.oldPath !== original.oldPath || current.base !== original.base || current.old.revision !== original.old.revision || current.new.revision !== original.new.revision);

const NotePath = (props: { path: string }) => {
  const cut = createCutTitle(() => props.path);

  return <p {...cut.attrs()} class="note-path" style={{ width: "fit-content", "max-width": "100%" }}>{props.path}</p>;
};

/** The keyboard-reachable batch remains available from the tree as well as the reader. */
export const NoteBatch = (props: { id: string; name: string; fallback?: () => void; notes: Notes; comparison?: FileComparison | undefined; edit: (note: Note) => void; recover: () => void }) => {
  const state = () => props.notes.state(props.id);

  const status = () => {
    const value = state();

    return value.kind === "pending" ? copy.submittingNotes : value.text;
  };

  const uncertain = () => state().kind === "unknown";
  const busy = () => state().kind === "pending" || uncertain();

  const older = (note: Note) => olderComparison(props.comparison, note.comparison);

  return <section class="note-batch" aria-label={copy.notesRegion}>
      <ReservedLine message={props.notes.problem() ?? props.notes.failure(props.id)} status={status()}>
        <Show when={props.notes.failure(props.id)}><button type="button" data-button="quiet" onClick={() => props.notes.dismiss(props.id)}>{copy.dismissFailure}</button></Show>
        <Show when={uncertain()}><button type="button" data-button="quiet" onClick={() => void props.notes.reconcile(props.id)}>{copy.checkMessageHistory}</button></Show>
        <Show when={props.notes.problem()}><button type="button" data-button="quiet" onClick={props.recover}>{copy.openNotesRecovery}</button></Show></ReservedLine>
      <Show when={props.notes.list(props.id).length}>
        <ul aria-label={copy.notes}><For each={props.notes.list(props.id)}>{(note, index) => <li>
          <NotePath path={note.comparison.newPath} />
          <p>{copy.noteLocation(note.comparison.newPath, note.side, note.start, note.end)}<ReservedLine status={older(note) ? copy.originalContext : null} /></p>
          <p class="note-text">{note.text}</p>
          <div class="diff-actions">
            <button type="button" data-button="quiet" aria-label={copy.editNote(index() + 1)} onClick={() => props.edit(note)}>{copy.editNoteAction}</button>
            <button type="button" data-button="quiet" aria-label={copy.removeNote(index() + 1)} onClick={event => {
              const focused = event.currentTarget === document.activeElement;
              const buttons = [...event.currentTarget.closest("ul")!.querySelectorAll<HTMLButtonElement>(".diff-actions button:last-child")];
              const at = buttons.indexOf(event.currentTarget);
              props.notes.remove(props.id, note.id);

              if (focused) {
                const next = buttons[at + 1] ?? buttons[at - 1];

                if (next?.isConnected) next.focus();
                else props.fallback?.();
              }
            }}>{copy.remove}</button>
          </div>
        </li>}</For></ul>
      </Show>
      <Show when={props.notes.list(props.id).length > 0}><button type="button" data-button="quiet" disabled={busy()} onClick={async event => {
        const focus = captureFocus();
        const button = event.currentTarget;
        const focused = document.activeElement === button;
        await props.notes.send(props.id);

        if (focused && (document.activeElement === button || document.activeElement === document.body) && props.notes.list(props.id).length === 0) focus(() => props.fallback?.());
      }}>{copy.sendNotesTo(props.name)}</button></Show>
    </section>;
};


/** Project-local access stays available when the original Agent or Worktree is gone. */
export const NoteRecovery = (props: { notes: Notes; available: (id: string) => boolean; name: (id: string) => string; open: boolean; onOpenChange: (open: boolean) => void }) => {
  const { app } = useConnectedProject();
  const [comparisons, setComparisons] = createSignal<ReadonlyMap<string, FileComparison>>(new Map());

  createEffect(on(() => props.open, open => {
    if (!open) return;
    let active = true;

    setComparisons(new Map());

    for (const [id, value] of props.notes.entries()) {
      if (!props.available(id)) continue;
      const paths = new Set([...value.forms, ...value.notes, ...value.submissions.flatMap(submission => submission.notes), ...value.recovered.flatMap(record => [...record.data.forms, ...record.data.notes])].map(note => note.comparison.newPath));

      for (const path of paths) void reply(() => app.rpc("agent.compareFile", { id, path })).then(result => {
        if (active && result.kind === "ok") setComparisons(previous => new Map(previous).set(`${id}:${path}`, result.value));
      });
    }

    onCleanup(() => { active = false; });
  }));

  const older = (id: string, comparison: FileComparison) => olderComparison(comparisons().get(`${id}:${comparison.newPath}`), comparison);

  const [editing, setEditing] = createSignal<{ id: string; form: NoteFormDraft }>();
  const [failures, setFailures] = createSignal<ReadonlyMap<string, string>>(new Map());

  const clear = (key: string) => setFailures(previous => {
    const next = new Map(previous);

    next.delete(key);

    return next;
  });

  createEffect(() => {
    const keys = new Set(["all", ...props.notes.entries().flatMap(([, value]) => [
      ...value.forms.map(form => form.revision), ...value.notes.map(note => note.revision),
      ...value.recovered.map(record => record.revision), ...value.submissions.map(submission => submission.revision),
    ]), ...props.notes.invalid().map(record => record.revision)]);

    if (props.notes.problem()) return;
    setFailures(previous => new Map([...previous].filter(([key]) => keys.has(key))));
  });
  let trigger: HTMLButtonElement | undefined;
  const close = () => { props.onOpenChange(false); setEditing(undefined); trigger?.focus(); };

  const copyValue = async (value: string, key: string) => {
    try { await writeText(app, value); clear(key); }
    catch (error) { if (!(error instanceof Error)) throw error; setFailures(previous => new Map(previous).set(key, clipboardFailure("copy", error))); }
  };

  const any = () => props.notes.entries().some(([id, value]) => (!props.available(id) && (value.notes.length || value.forms.length || value.submissions.length)) || value.submissions.some(submission => submission.status !== "pending") || value.recovered.length) || props.notes.problem();

  createEffect(() => { if (!props.open) setEditing(undefined); });
  createEffect(() => { if (!props.notes.entries().length && !props.notes.problem()) props.onOpenChange(false); });

  return <Show when={any() || props.open}><Popover.Root closeOnInteractOutside={false} open={props.open} onOpenChange={({ open: next }) => { if (next) props.onOpenChange(true); else close(); }} closeOnEscape positioning={{ placement: "top-end", strategy: "fixed" }}>
    <Popover.Trigger asChild={attributes => <button {...attributes()} ref={element => { trigger = element; }} type="button" data-button="quiet">{copy.recoverNotes}</button>} />
    <Popover.Positioner><Popover.Content class="worktree-panel note-recovery" aria-label={copy.recoverNotes}>
      <header class="diff-header"><span class="diff-identity">{copy.recoverNotes}</span><button type="button" data-button="quiet" onClick={close}>{copy.close}</button></header>
      <ReservedLine message={props.notes.problem()} />
      <For each={[...failures()]}>{([key, text]) => <ReservedLine message={text}><button type="button" data-button="quiet" onClick={() => clear(key)}>{copy.dismissFailure}</button></ReservedLine>}</For>
      <div class="diff-actions"><button type="button" data-button="quiet" onClick={() => void copyValue(props.notes.copy(), "all")}>{copy.copy}</button><Show when={props.notes.problem()}><button type="button" data-button="quiet" onClick={props.notes.retry}>{copy.retry}</button></Show></div>
      <Show when={editing()} keyed>{value => <NoteForm id={value.id} comparison={comparisons().get(`${value.id}:${value.form.comparison.newPath}`) ?? value.form.comparison} notes={props.notes} recovered={value.form} selection={{ side: value.form.side, start: Number(value.form.start), end: Number(value.form.end) }} close={() => setEditing(undefined)} />}</Show>
      <For each={props.notes.entries()}>{([id, value]) => <section class="note-batch">
        <h3>{copy.noteDraftStatus(props.name(id), value.submissions[0]?.status ?? copy.noteDraft)}</h3>
        <ReservedLine status={!props.available(id) ? copy.originalRecipientUnavailable : null} />
        <For each={value.forms}>{form => <div>
          <p class="note-path">{form.comparison.newPath}<Show when={older(id, form.comparison)}>{copy.originalContext}</Show></p><p>{copy.noteLocation(form.comparison.newPath, form.side, Number(form.start), Number(form.end))}</p><p class="note-text">{form.text}</p>
          <div class="diff-actions"><button type="button" data-button="quiet" onClick={() => void copyValue(JSON.stringify({ project: props.notes.project, recipient: id, form }, null, 2), form.revision)}>{copy.copy}</button><Show when={props.available(id)}><button type="button" data-button="quiet" onClick={() => setEditing({ id, form })}>{copy.resumeNote}</button></Show><button type="button" data-button="quiet" onClick={() => { props.notes.discardForm(id, form.revision); setEditing(undefined); }}>{copy.discardDraft}</button></div>
        </div>}</For>
        <For each={value.notes}>{note => <div>
          <p class="note-path">{note.comparison.newPath}<Show when={older(id, note.comparison)}>{copy.originalContext}</Show></p><p>{copy.noteLocation(note.comparison.newPath, note.side, note.start, note.end)}</p><p class="note-text">{note.text}</p>
          <div class="diff-actions"><button type="button" data-button="quiet" onClick={() => void copyValue(JSON.stringify({ project: props.notes.project, recipient: id, note }, null, 2), note.revision)}>{copy.copy}</button><button type="button" data-button="quiet" onClick={() => props.notes.remove(id, note.id)}>{copy.discardDraft}</button></div>
        </div>}</For>
        <For each={value.recovered}>{recovered => <div><RecordPreview older={comparison => older(id, comparison)} rows={[...recovered.data.forms, ...recovered.data.notes, ...recovered.data.submissions.flatMap(submission => submission.notes)]} /><div class="diff-actions"><button type="button" data-button="quiet" onClick={() => void copyValue(JSON.stringify({ project: props.notes.project, recipient: id, ...recovered }, null, 2), recovered.revision)}>{copy.copy}</button><button type="button" data-button="quiet" onClick={() => props.notes.discardRecovered(id, recovered.revision)}>{copy.discardDraft}</button></div></div>}</For>
        <For each={value.submissions}>{submission => <div>
          <ReservedLine status={`${copy.noteDraftStatus(props.name(id), submission.status)} ${submission.text}`} /><RecordPreview older={comparison => older(id, comparison)} rows={submission.notes} />
          <div class="diff-actions"><button type="button" data-button="quiet" onClick={() => void copyValue(submission.body, submission.revision)}>{copy.copy}</button><Show when={submission.status === "unknown"}><button type="button" data-button="quiet" onClick={() => void props.notes.reconcile(id)}>{copy.checkMessageHistory}</button></Show><Show when={submission.status !== "pending"}><button type="button" data-button="quiet" onClick={() => props.notes.discardSubmission(id, submission.revision)}>{copy.discardSubmission}</button></Show></div>
        </div>}</For>
      </section>}</For>
      <For each={props.notes.invalid()}>{draft => <div><RecordPreview rows={draft.preview} /><div class="diff-actions"><button type="button" data-button="quiet" onClick={() => void copyValue(JSON.stringify(draft), draft.revision)}>{copy.copy}</button><button type="button" data-button="quiet" onClick={() => props.notes.discardInvalid(draft.revision)}>{copy.discardDraft}</button></div></div>}</For>
    </Popover.Content></Popover.Positioner>
  </Popover.Root></Show>;
};

/** Recovery uses the same path, range and text presentation as live notes. */
const RecordPreview = (props: { rows: readonly (Note | NoteFormDraft)[]; older?: (comparison: FileComparison) => boolean | undefined }) => <For each={props.rows} fallback={<p>{copy.unreadableNoteDraft}</p>}>{note => <div><p class="note-path">{note.comparison.newPath}<Show when={props.older?.(note.comparison)}>{copy.originalContext}</Show></p><p>{copy.noteLocation(note.comparison.newPath, note.side, Number(note.start), Number(note.end))}</p><p class="note-text">{note.text}</p></div>}</For>;
