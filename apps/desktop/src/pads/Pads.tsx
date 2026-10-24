import { captureFocus, queueFocus } from "../keys/focus";
import { writeText } from "../app/clipboard";
import { contextMenu, linkEntries, openOnRelease } from "../ui/contextMenu";
import * as v from "valibot";
import { Editable } from "../ui/Editable";
import { RpcError, INTERNAL_CODE } from "../app/seam";
import { createCutTitle } from "../ui/cutTitle";
import { activationSource } from "../keys/registry";
import { copy } from "../copy";
import { createSelection } from "../shelf/selection";
import { shelfRowKeys } from "../shelf/keys";
import { createBulkDelete } from "../shelf/BulkDelete";
import { failureOf } from "../todos/failureOf";
import { OwnerMark } from "../ink/OwnerMark";
import { Icon } from "../ink/Icon";
import { EmptyState } from "../ink/EmptyState";

import { createEffect, createMemo, createSignal, For, onCleanup, Show } from "solid-js";
import type { Accessor, Setter } from "solid-js";
import type { Pad } from "@contracts/pad/Pad";
import { ReservedLine } from "../ink/ReservedLine";
import { ShelfRow } from "../ink/ShelfRow";
import { type ConnectedProject, useConnectedProject } from "../state/connectedProject";
import { createFailure } from "./failure";
import { PadDrawer } from "./PadDrawer";
import { USER } from "./owner";
import { shownPads } from "./shownPads";
import { shelfScope } from "../shelf/scope";
import { ScopeAction } from "../shelf/ScopeAction";
import { OwnerGroups } from "../shelf/OwnerGroups";
import { OwnerPicker } from "../todos/OwnerPicker";
import { ownerGroups, ownersOnRail, sameOwner } from "../shelf/scope";
import { Sortable } from "../ui/sortable";
import { CHORDS, TEXT_FIELDS } from "../keys/chords";
import { registerKeys } from "../keys/registry";
import { shelfKeyScope } from "../shelf/keys";
import type { Actor } from "@contracts/Actor";
import type { OwnerTarget } from "../todos/sortedList";
import { usePadList } from "../rail/pads/padList";
import type { PadList } from "../rail/pads/padList";

/** The Shelf's `pads` list; `pad.list` logs no Touch, so refetching it on every `pad.changed` leaves no trace in Provenance. One fetch serves the list and any open Drawer. */
export const Pads = (props: {
  /** Open this Project's retained creation revision without submitting it. */
  recovery?: { project: string; revision: string; list: PadList };
}) => {
  const connected = useConnectedProject();

  if (props.recovery && props.recovery.project !== connected.project.path) return null;

  const list = props.recovery?.list ?? usePadList();
  const pads = list.pads;
  const creation = { surface: "pads", object: "creation", recipient: null };

  const retentionProblem = () => {
    const problem = connected.drafts.problem();

    return problem && (!problem.destination || (problem.destination.surface === creation.surface && problem.destination.object === creation.object)) ? problem : invalid() ? { cause: "invalid" } : null;
  };

  const nameLabel = copy.copyField(copy.draftField("name").toLowerCase());
  const restored = connected.drafts.list().filter(draft => draft.destination.surface === "pads" && draft.destination.object === "creation" && draft.destination.recipient === null && (props.recovery?.revision === undefined || draft.revision === props.recovery?.revision)).at(-1);
  const [naming, setNaming] = createSignal(props.recovery?.revision !== undefined && restored !== undefined);
  createEffect(() => { if (naming()) onCleanup(connected.drafts.editor(creation)); });
  const parsed = restored ? v.safeParse(v.object({ name: v.string(), home: v.nullable(v.string()) }), restored.value) : null;
  const [invalid, setInvalid] = createSignal(Boolean(parsed && !parsed.success));
  const [name, setName] = createSignal(parsed?.success ? parsed.output.name : "");
  let revision = restored?.revision;
  let field: HTMLInputElement | undefined;
  let home = parsed?.success ? parsed.output.home : null;
  let caret = restored?.selection ?? null;
  let captured = restored ? JSON.stringify([restored.value.name, restored.value.home, caret]) : null;

  const capture = () => {
    if (invalid()) return;
    caret = field ? { start: field.selectionStart ?? 0, end: field.selectionEnd ?? 0, direction: field.selectionDirection ?? "none" } : caret;
    const fingerprint = JSON.stringify([name(), home, caret]);

    if (captured === fingerprint) return;
    captured = fingerprint;
    revision = connected.drafts.edit(creation, { name: name(), home }, caret, "ordinary", revision ?? null).draft.revision;
  };

  let plusButton: HTMLButtonElement | undefined;
  const failure = createFailure();
  const [itemFailure, setItemFailure] = createSignal<{ incarnation: string; message: string } | null>(null);

  const reportItemFailure = (pad: Pad, message: string | null) => {
    if (message !== null) {
      // A late reply for a deleted source must not resurrect its error on the list.
      if (pads().some(current => current.incarnation === pad.incarnation)) setItemFailure({ incarnation: pad.incarnation, message });
    } else {
      if (itemFailure()?.incarnation === pad.incarnation) setItemFailure(null);
      reportAssignmentFailure(pad.incarnation, null);
    }
  };

  createEffect(() => {
    const failed = itemFailure();

    if (failed && !pads().some(pad => pad.incarnation === failed.incarnation)) setItemFailure(null);
  });
  const scope = shelfScope(connected);

  const create = () =>
    failure.run(async () => {
      if (!name().trim()) return;
      capture();
      const submitted = revision ? connected.drafts.submit(revision) : null;

      if (!submitted) return;
      const target = name().trim();
      const targetHome = home;
      const maker = scope.agent();

      const timeout = setTimeout(() => {
        connected.drafts.settle(submitted.draft.revision, "unknown");
        failure.show(copy.draftStatus.unknown);
      }, 15_000);

      try {
        await connected.app.rpc("pad.create", { name: target, text: null });
        connected.drafts.settle(submitted.draft.revision, "acknowledged");

        if (maker === null && revision === submitted.draft.revision) {
          revision = undefined;
          captured = null;
          setName("");
        }

        // U3735: under a selected Agent the new Pad is that Agent's, before it moves Home; the name stays in the field until that holds.
        if (maker !== null) {
          try {
            await connected.app.rpc("pad.setOwner", { name: target, owner: maker });
          } catch (error) {
            if (!(error instanceof Error)) throw error;
            throw new Error(copy.padOwnerRefused(error.message), { cause: error });
          }

          if (revision === submitted.draft.revision) {
            revision = undefined;
            captured = null;
          }

          if (name() === target) setName("");
        }

        // The creation reply acknowledges only its captured name; a later edit remains current.
        if (targetHome !== null) await connected.app.rpc("pad.move", { name: target, home: targetHome });
      } catch (error) {
        connected.drafts.settle(submitted.draft.revision, error instanceof RpcError && error.code !== INTERNAL_CODE ? "failed" : "unknown");
        throw error;
      } finally {
        clearTimeout(timeout);
      }
    });

  const makeYours = async (name: string) => {
    const pad = pads().find(pad => pad.name === name);

    if (!pad) return;
    reportItemFailure(pad, await failureOf(async () => {
      await connected.app.rpc("pad.setOwner", { name, owner: USER });
      await list.reload();
    }));
  };

  const shown = shownPads(connected, list);

  const selection = createSelection(() => shown().map((pad) => pad.name), () => connected.rail.selected() ?? "overview", name => rowButtonOf(name));

  const bulk = createBulkDelete({
    app: connected.app,
    selected: selection.selected,
    afterRemoval: selection.afterRemoval,
    targets: () => shown().map((pad) => ({ id: pad.name, label: pad.name, owner: pad.owner.kind === "agent" ? connected.rail.nameOf(pad.owner) : undefined })),
    context: () => connected.rail.selected() ?? "overview",
    noun: copy.pads, replace: selection.replace, clear: selection.clear, cancelSelection: selection.cancel,
    remove: (name) => failureOf(() => connected.app.rpc("pad.delete", { name })),
    reload: async () => {
      await list.reload();

      return list.failure();
    },
  });

  shelfRowKeys({
    focused: selection.focused,
    exportPad: async name => {
      const pad = pads().find(pad => pad.name === name);

      if (!pad) return;

      const message = await failureOf(async () => {
        const path = await connected.app.chooseSavePath([name, ".md"].join(""));

        if (path !== null) await connected.app.rpc("pad.export", { name, path });
      });

      if (message !== null) reportItemFailure(pad, message);
    },
    remove: name => bulk.ask(false, name),
  });

  const [dragging, setDragging] = createSignal(false);
  const [picking, pick] = createSignal<{ id: string; anchor: HTMLElement; focus?: ReturnType<typeof captureFocus> } | null>(null);
  const setPicking = (request: { id: string; anchor: HTMLElement } | null) => { pick(request && { ...request, focus: captureFocus(true) }); };

  const pickFails = new Map<string, { message: string | null; show: (message: string | null) => void }>();

  const reportAssignmentFailure = (incarnation: string, message: string | null) => {
    const prior = pickFails.get(incarnation);

    if (!prior || !pads().some(pad => pad.incarnation === incarnation)) return;
    prior.message = message;
    prior.show(message);
  };

  createEffect(() => {
    const alive = new Set(pads().map(pad => pad.incarnation));

    for (const incarnation of pickFails.keys()) if (!alive.has(incarnation)) pickFails.delete(incarnation);
  });
  const rowButtons = new Map<string, HTMLButtonElement>();
  const padOf = (name: string) => pads().find(pad => pad.name === name);

  const rowButtonOf = (name: string) => document.querySelector<HTMLButtonElement>(`.pads-tab [data-pad="${CSS.escape(name)}"] .pad-name`);

  /** U3736: the owner list closes with focus back on the row, found again after the refetch. */
  const stopPicking = () => {
    const name = picking()?.id;

    setPicking(null);

    if (name !== undefined) queueFocus(() => rowButtonOf(name)?.focus());
  };

  /** U3736: the one `pad.setOwner` of a drop or a choice; a refusal shows on the row and the Pad stays in its group. */
  const assign = async (name: string, owner: Actor | null, focus: boolean) => {
    const source = padOf(name);

    if (owner === null || !source) return;
    const permission = captureFocus();
    const message = await failureOf(() => connected.app.rpc("pad.setOwner", { name, owner }));

    reportAssignmentFailure(source.incarnation, message);

    // U138: the refetch replaces the row that had focus; give it back once the Pad has arrived.
    if (message === null && focus) setRegrouped({ name, owner, focus: permission });
  };

  const [regrouped, setRegrouped] = createSignal<{ name: string; owner: Actor; focus: ReturnType<typeof captureFocus> } | null>(null);

  createEffect(() => {
    const request = regrouped();
    const arrived = request && padOf(request.name);

    if (!request || !arrived || !sameOwner(arrived.owner, request.owner)) return;
    queueMicrotask(() => {
      const kept = document.activeElement;

      if (kept && kept !== document.body && kept.isConnected && !kept.closest("[role=menu]")) return setRegrouped(null);
      const button = rowButtonOf(request.name);

      if (button) { request.focus(() => button.focus()); setRegrouped(null); }
    });
  });

  registerKeys(shelfKeyScope, [{
    action: CHORDS.find(row => row.keys === "O")!,
    capture: (focus) => {
      const row = focus instanceof HTMLElement && focus.closest(".pad-name") ? focus.closest<HTMLElement>("[data-pad]") : null;
      const name = row?.dataset.pad;
      const anchor = focus instanceof HTMLElement && !focus.matches(TEXT_FIELDS) ? focus : null;

      return { available: () => name !== undefined && padOf(name) !== undefined && ownersToOffer(connected, padOf(name)!.owner).length > 0 && anchor !== null, invoke: () => { if (anchor && name !== undefined) setPicking({ id: name, anchor }); } };
    },
  }]);

  const offers = () => {
    const current = padOf(picking()?.id ?? "");

    return current ? ownersToOffer(connected, current.owner) : [];
  };

  const dropOwner = {
    target: (name: string, x: number, y: number) => {
      const own = padOf(name);

      for (const element of document.querySelectorAll<HTMLElement>(".pads-tab [data-shelf-group]")) {
        const box = element.getBoundingClientRect();

        if (x < box.left || x >= box.right || y < box.top || y >= box.bottom) continue;
        const group = element.dataset.shelfGroup!;
        const found = ownersOnRail(connected).find(each => each.id === group);

        // Unassigned is no owner a Pad can take, the own group is no change, and a group that left the Rail takes nothing.
        if (!found || found.actor === null || !own || sameOwner(found.actor, own.owner)) return null;

        return { element, attribute: "data-drop-target", owner: found.actor };
      }

      return null;
    },
  };

  const isEmpty = createMemo(() => list.loaded() && pads().length === 0);

  return (
    <section aria-label={copy.pads} class="pads-tab">
      <ScopeAction tab="pads">
        <button
          data-button="add"
          type="button"
          aria-label={copy.addPad}
          class="shelf-add"
          ref={(button) => (plusButton = button)}
          onClick={() => {
            failure.clear();

            if (!revision) home = scope.home();
            setNaming(true);
          }}
        >
          <Icon name="plus" />
          <span>{copy.pad}</span>
        </button>
      </ScopeAction>
      <ReservedLine message={itemFailure()?.message ?? failure.message() ?? list.failure() ?? (retentionProblem() ? copy.draftRetention(connected.project.name, copy.pad, retentionProblem()!.cause) : null)}>
        <Show when={itemFailure()}><button data-button="quiet" type="button" onClick={() => setItemFailure(null)}>{copy.dismiss}</button></Show>
        <Show when={retentionProblem()}>
        <button data-button="primary" type="button" onClick={() => connected.drafts.retry()}>{copy.storageRetry}</button>
        <button data-button="quiet" type="button" onClick={() => void writeText(connected.app, name()).then(() => failure.clear()).catch(error => failure.show(copy.copyDraftFailure(String(error))))}>{nameLabel}</button>
        <button data-button="quiet" type="button" onClick={() => { if (revision) connected.drafts.discard(revision); revision = undefined; captured = null; setName(""); setInvalid(false); }}>{copy.discardRevision}</button>

      </Show></ReservedLine>
      <Show when={isEmpty() && !naming()}>
        <EmptyState icon="order" heading={copy.noPadsYet} hint={copy.noPadsHint} />
      </Show>
      <OwnerGroups groups={() => ownerGroups(connected, shown(), pad => pad.owner)} dragging={dragging} bare={() => connected.rail.nodes.find(node => node.id === connected.rail.selected())?.kind === "agent" ? connected.rail.selected() : null} adding={() => null} reveal={() => null}>{group => <PadRows
        selection={selection}
        bulk={bulk}
        pads={pads}
        shown={() => group().items}
        onMakeYours={(name) => void makeYours(name)}
        onItemFailure={reportItemFailure}
        onAssign={(name, anchor) => setPicking({ id: name, anchor })}
        failWith={(incarnation, show) => {
          const message = pickFails.get(incarnation)?.message ?? null;

          pickFails.set(incarnation, { message, show });
          show(message);
        }}
        clearFailure={incarnation => reportAssignmentFailure(incarnation, null)}
        rowButtons={rowButtons}
        external={{
          attribute: "data-pad-drop",
          target: dropOwner.target,
          onDropOutside: (name, target) => void assign(name, target.owner, true),
        }}
        onDrag={setDragging}
      />}</OwnerGroups>
      <OwnerPicker label={name => copy.ownersForPad(name ?? "")} target={picking} offers={offers} choose={(name, owner) => void assign(name, owner, true)} close={stopPicking} />
      <Show when={naming()}>
        <div class="shelf-list">
          <ShelfRow glyph={<span class="light"><OwnerMark owner={USER} decorative /></span>} class="entry-row pad-inset">
            <Editable.Root edit value={name()} readOnly={invalid()} selectOnFocus={false} submitMode="none" onValueChange={({ value }) => { setName(value); capture(); }}>
            <Editable.Input type="text" class="entry-field shelf-title" aria-label={copy.padName} placeholder={copy.newPadPlaceholder} value={name()}
              ref={(input) => {
                field = input;
                queueFocus(() => {
                  if (!input.isConnected) return;
                  input.focus();

                  if (caret) input.setSelectionRange(caret.start, caret.end, caret.direction);
                  else if (restored) {
                    // Focus supplies an uncaptured caret; retaining it would replace the current creation draft.
                    caret = { start: input.selectionStart ?? 0, end: input.selectionEnd ?? 0, direction: input.selectionDirection ?? "none" };
                    captured = JSON.stringify([name(), home, caret]);
                  }
                });
              }}
              onInput={(event) => { setName(event.currentTarget.value); capture(); }}
              onSelect={() => { if (revision) capture(); }}
              on:keydown={(event) => {
                if (event.key === "Enter" && name().trim()) { event.preventDefault(); void create(); }
                else if (event.key === "Escape" || (event.key === "Enter" && !name().trim())) {
                  event.preventDefault();
                  setNaming(false);
                  plusButton?.focus();
                }
              }}
              onBlur={() => { if (!name().trim()) setNaming(false); }} />
            </Editable.Root>
          </ShelfRow>
        </div>
      </Show>

      <bulk.Bar />
    </section>
  );
};

const PadRows = (props: {
  selection: ReturnType<typeof createSelection<string>>;
  bulk: ReturnType<typeof createBulkDelete<string>>;
  pads: Accessor<readonly Pad[]>;
  shown: Accessor<readonly Pad[]>;
  onMakeYours: (name: string) => void;
  onItemFailure: (pad: Pad, message: string | null) => void;
  onAssign: (name: string, anchor: HTMLElement) => void;
  failWith: (incarnation: string, show: (message: string | null) => void) => void;
  clearFailure: (incarnation: string) => void;
  rowButtons: Map<string, HTMLButtonElement>;
  external: { attribute: string; target: (name: string, x: number, y: number) => OwnerTarget | null; onDropOutside: (name: string, target: OwnerTarget) => void };
  onDrag: Setter<boolean>;
}) => (
    <Sortable.Root class="shelf-list" plain ids={props.shown().map(pad => pad.name)} external={props.external} onDrag={props.onDrag}>
      <For each={props.shown()}>
        {(pad, at) => (
          <Sortable.Item id={pad.name} index={at()} pressable>{parts => <PadRow pad={pad} parts={parts} {...props} />}</Sortable.Item>
        )}
      </For>
    </Sortable.Root>
);

const ownersToOffer = (connected: ConnectedProject, current: Actor) => ownersOnRail(connected).filter(owner => owner.actor !== null && !sameOwner(owner.actor, current));

const PadRow = (props: Parameters<typeof PadRows>[0] & { pad: Pad; parts: Parameters<Parameters<typeof Sortable.Item>[0]["children"]>[0] }) => {
  const connected = useConnectedProject();
  const pad = props.pad;
  const nameCut = createCutTitle(() => pad.name);
  const [failed, setFailed] = createSignal<string | null>(null);

  props.failWith(pad.incarnation, setFailed);

  return (
          <ShelfRow
            ref={props.parts.ref}
            data-pad={pad.name}
            data-selected={props.selection.selected().includes(pad.name) ? "true" : undefined}
            data-shelf-row
            onContextMenu={(event: MouseEvent) => openOnRelease(event, (x, y) => void contextMenu(connected.app, [
              ...linkEntries(connected.app, { kind: "pad", id: pad.name }, connected.rail.nodes, (node) => connected.rail.exitOf(node) === null, message => props.onItemFailure(pad, message)),
              { id: "delete", label: copy.delete, run: () => { props.selection.replace([pad.name]); props.bulk.ask(); } },
            ], x, y))}
            class="pad-inset"
            glyph={
              <Show
                when={pad.owner.kind === "user"}
                fallback={
                  <button data-button="icon"
                    type="button"
                    class="word"
                    aria-label={copy.makePadYours(pad.name)}
                    onClick={() => props.onMakeYours(pad.name)}
                  >
                    <OwnerMark owner={pad.owner} />
                  </button>
                }
              >
                <span class="light">
                  <OwnerMark owner={pad.owner} />
                </span>
              </Show>
            }
          >
            <div class="pad-row-head">
            <button
              type="button"
              class="pad-name"
              aria-pressed={props.selection.selected().includes(pad.name)}
              ref={(row) => {
                props.selection.register(pad.name, row);
                props.parts.handleRef(row ?? undefined);

                if (row) props.rowButtons.set(pad.name, row);
                else props.rowButtons.delete(pad.name);
              }}
              on:keydown={(event) => {
                props.bulk.key(event);

                if (!event.defaultPrevented && !props.bulk.pending()) props.selection.key(pad.name, event);
              }}
              onClick={(event) => !props.bulk.pending() && props.selection.click(pad.name, event, () =>
                connected.drawer.open(() => (
                  <PadDrawer
                    name={pad.name}
                    pads={props.pads}
                    onLateFailure={message => props.onItemFailure(pad, message)}
                    onFailureCleared={() => props.onItemFailure(pad, null)}
                  />
                ), activationSource(event)))
              }
            >
              <span {...nameCut.attrs()} class="shelf-title">{pad.name}</span>
            </button>
              {/* oxlint-disable-next-line anti-slop/no-tooltip-bypass -- U171 requires a focusable disabled button to expose its reason as title. */}
              <button data-button="row-action" type="button" class="word light pad-assign" aria-label={copy.assignPadNamed(pad.name)} aria-disabled={ownersToOffer(connected, pad.owner).length === 0 || undefined} title={ownersToOffer(connected, pad.owner).length === 0 ? copy.noOtherOwners : undefined} onClick={(event) => { if (ownersToOffer(connected, pad.owner).length > 0) props.onAssign(pad.name, event.currentTarget); }}>{copy.assign}</button>
            </div>
            <p class="shelf-meta">{copy.ownedBy}{connected.rail.nameOf(pad.owner)}</p>
            <ReservedLine overlay message={failed()}><button data-button="quiet" type="button" onClick={() => props.clearFailure(pad.incarnation)}>{copy.dismiss}</button></ReservedLine>
          </ShelfRow>
  );
};
