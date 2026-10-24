import { queueFocus } from "../keys/focus";
import { ReservedLine } from "../ink/ReservedLine";
import { copy } from "../copy";
import { Tabs } from "../ui/Tabs";
import { createSignal, Show, untrack } from "solid-js";
import type { FileComparison } from "@contracts/agent/FileComparison";
import type { Note, Notes, NoteFormDraft } from "./notes";

/** Keyboard selection uses the same source and inclusive range as Pierre's gutter selection. */
export const NoteForm = (props: { id: string; comparison: FileComparison; notes: Notes; selection: { side: "old" | "new"; start: number; end: number }; editing?: Note | undefined; recovered?: NoteFormDraft | undefined; close: () => void }) => {
  const restored = untrack(() => props.recovered ?? props.notes.form(props.id, props.comparison, props.editing?.id));
  const comparison = restored?.comparison ?? props.editing?.comparison ?? props.comparison;
  const revision = restored?.revision ?? crypto.randomUUID();
  const replacing = restored?.replacing ?? props.editing?.id;
  const [side, setSide] = createSignal(restored?.side ?? props.editing?.side ?? props.selection.side);
  const [start, setStart] = createSignal(restored?.start ?? String(props.editing?.start ?? props.selection.start));
  const [end, setEnd] = createSignal(restored?.end ?? String(props.editing?.end ?? props.selection.end));
  const [text, setText] = createSignal(restored?.text ?? props.editing?.text ?? "");
  let field: HTMLTextAreaElement | undefined;

  const persist = () => props.notes.editForm(props.id, {
    revision, comparison, replacing: replacing ?? null, side: side(), start: start(), end: end(), text: text(),
    selection: field ? { start: field.selectionStart, end: field.selectionEnd, direction: field.selectionDirection } : restored?.selection ?? props.editing?.selection ?? null,
  });

  const lines = () => {
    const content = comparison[side()];

    if (content.kind !== "text" || content.text === "") return [];
    const result = content.text.split("\n");

    if (result.at(-1) === "") result.pop();

    return result;
  };

  const valid = () => text().trim() !== "" && start() !== "" && end() !== "" && Number.isInteger(Number(start())) && Number.isInteger(Number(end())) && Number(start()) >= 1 && Number(end()) >= Number(start()) && Number(end()) <= lines().length && (replacing !== undefined || props.notes.list(props.id).length < 200);

  return <form class="note-form" aria-label={copy.lineNote} onSubmit={event => {
    event.preventDefault();

    if (valid() && props.notes.save(props.id, { comparison, side: side(), start: Number(start()), end: Number(end()), source: lines().slice(Number(start()) - 1, Number(end())).join("\n"), text: text(), selection: field ? { start: field.selectionStart, end: field.selectionEnd, direction: field.selectionDirection } : null }, replacing, revision)) props.close();
  }}>
    <Tabs.Root value={side()} onValueChange={({ value }) => { setSide(value === "old" ? "old" : "new"); persist(); }}>
      <Tabs.List aria-label={copy.noteSide}><Tabs.Trigger value="old">{copy.oldSide}</Tabs.Trigger><Tabs.Trigger value="new">{copy.newSide}</Tabs.Trigger></Tabs.List>
      <Tabs.Content value="old">{copy.selectOldLines}</Tabs.Content><Tabs.Content value="new">{copy.selectNewLines}</Tabs.Content>
    </Tabs.Root>
    <label>{copy.startLine}<input type="number" min="1" value={start()} onInput={event => { setStart(event.currentTarget.value); persist(); }} /></label>
    <label>{copy.endLine}<input type="number" min="1" value={end()} onInput={event => { setEnd(event.currentTarget.value); persist(); }} /></label>
    <label class="note-field">{copy.note}<textarea value={text()} maxLength={20000} onInput={event => { setText(event.currentTarget.value); persist(); }} onSelect={() => { if (text() !== "" || restored || replacing !== undefined) persist(); }} ref={element => {
      field = element;
      queueFocus(() => {
        if (!element.isConnected) return;
        element.focus();

        const selection = restored?.selection ?? props.editing?.selection;

        if (selection) element.setSelectionRange(selection.start, selection.end, selection.direction);
      });
    }} /></label>
    <ReservedLine status={(comparison.oldPath !== props.comparison.oldPath || comparison.newPath !== props.comparison.newPath || comparison.base !== props.comparison.base || comparison.old.revision !== props.comparison.old.revision || comparison.new.revision !== props.comparison.new.revision ? copy.originalContextStatus : null) || (props.notes.list(props.id).length >= 200 ? copy.notesFull : copy.chooseLines) || null} />
    <div class="diff-actions"><button type="button" data-button="quiet" onClick={props.close}>{copy.cancel}</button><button type="submit" data-button="quiet" disabled={!valid()}>{copy.save}</button></div>
  </form>;
};
