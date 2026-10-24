import { KeyHint } from "./KeyHint";
import { copy } from "../copy";
import { createSignal, For } from "solid-js";
import { CHORDS, HELP_KEYS, LEADER } from "./chords";
import { placeOf } from "./places";
import { registerKeys } from "./registry";
import { Dialog } from "../ui/Dialog";

const bubbleScopes = new Map(Object.entries(copy.helpBubbleScope));

const rowScopes = new Map(Object.entries(copy.helpRowScope));

const sections = () => [
  { title: copy.helpScope.app, rows: CHORDS.filter(row => !row.inRail && !row.inTerminals && !row.inShelf) },
  { title: copy.helpScope.leader, rows: LEADER.map(row => ({ ...row, keys: `${copy.leaderChord} ${row.keys}`, what: bubbleScopes.get(row.keys) ?? row.what })) },
  { title: copy.helpScope.terminals, rows: CHORDS.filter(row => row.inTerminals) },
  { title: copy.helpScope.rail, rows: CHORDS.filter(row => row.inRail) },
  { title: copy.helpScope.shelf, rows: CHORDS.filter(row => row.inShelf && !row.inTriage && row.keys !== "T").map(row => ({ ...row, what: rowScopes.get(row.keys) ?? row.what })) },
  { title: copy.helpScope.todos, rows: CHORDS.filter(row => row.inShelf && row.keys === "T") },
  { title: copy.helpScope.triage, rows: CHORDS.filter(row => row.inTriage) },
];

const inTextField = (target: Element): boolean => target.closest('input, textarea, select, [contenteditable="true"]') !== null;

/** `?` opens Help from the Rail or the pane's frame, never from a text field, a Terminal, a Drawer or the switcher (U54). */
const opensFrom = (target: EventTarget | null): boolean => {
  if (!(target instanceof Element) || inTextField(target) || target.closest(".pane-screen")) return false;

  const place = placeOf(target);

  return place === "rail" || place === "pane";
};

/** The panel `?` opens over the centre pane, one line per chord. It holds focus while open; Esc, `?` or a click outside closes it and focus returns to where it was. */
export const Help = () => {
  const [open, setOpen] = createSignal(false);
  let panel: HTMLElement | undefined;
  let returnTo: Element | null = null;

  const show = (): void => {
    returnTo = document.activeElement;
    setOpen(true);
  };

  const close = (): void => {
    setOpen(false);

    if (returnTo instanceof HTMLElement && returnTo.isConnected) returnTo.focus({ preventScroll: true });
  };

  registerKeys({ kind: "app" }, [{ chords: HELP_KEYS, when: (press) => opensFrom(press.target), run: show }]);
  registerKeys({ kind: "layer", element: () => panel, active: open, modal: true }, [
    { chords: [...HELP_KEYS, "Escape"], run: close },
  ]);

  return (
    <Dialog.Root open={open()} onOpenChange={({ open }) => { if (!open) close(); }}
      initialFocusEl={() => panel ?? null}
      finalFocusEl={() => returnTo instanceof HTMLElement && returnTo.isConnected ? returnTo : null}
      onEscapeKeyDown={(event) => event.stopImmediatePropagation()}>
      <Dialog.Positioner><Dialog.Content class="help" aria-label={copy.help} ref={(element) => { panel = element; }}>
        <For each={sections()}>{section => <section class="help-section">
          <h2>{section.title}</h2>
          <For each={section.rows}>{chord => <p class="help-line"><KeyHint keys={chord.keys}>{chord.what}</KeyHint></p>}</For>
        </section>}</For>
        <section class="help-shelf" aria-label={copy.shelfKeys}>
        <h2>{copy.shelfKeys}</h2>
        <p><KeyHint keys={copy.previousNextKeys}>{copy.focusThePreviousNextShelfRow}</KeyHint></p>
        <p><KeyHint keys={copy.enter}>{copy.toggleTheFocusedShelfRowSelection}</KeyHint></p>
        <p><KeyHint keys={copy.rangeKeys}>{copy.selectARangeOfShelfRows}</KeyHint></p>
        <p><KeyHint keys={copy.esc}>{copy.deleteSelectedShelfRowsCancelSelection}</KeyHint></p>
        </section>
        <div class="app-dialog-actions"><Dialog.CloseTrigger data-button="quiet">{copy.close}</Dialog.CloseTrigger></div>
      </Dialog.Content></Dialog.Positioner>
    </Dialog.Root>
  );
};
