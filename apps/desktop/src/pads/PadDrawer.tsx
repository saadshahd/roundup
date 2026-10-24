import { captureFocus } from "../keys/focus";
import { writeText } from "../app/clipboard";
import { NativeAlert } from "../ui/NativeAlert";
import * as v from "valibot";
import { Tip } from "../ui/Tip";
import { createCutTitle } from "../ui/cutTitle";
import { IconButton } from "../ui/IconButton";
import type { ActivationSource } from "../keys/registry";
import { debounce } from "@solid-primitives/scheduled";
import { copy } from "../copy";
import { Icon } from "../ink/Icon";
import { createEffect, createMemo, createSignal, lazy, on, onCleanup, Show, Suspense } from "solid-js";
import type { Accessor } from "solid-js";
import type { Actor } from "@contracts/Actor";
import type { Pad } from "@contracts/pad/Pad";
import { ItemDrawer } from "../drawer/ItemDrawer";
import { ReservedLine } from "../ink/ReservedLine";
import { useConnectedProject } from "../state/connectedProject";
import { failureOf } from "../todos/failureOf";
import { createFailure } from "./failure";
import { RpcError, INTERNAL_CODE } from "../app/seam";
import { usePadList } from "../rail/pads/padList";
import type { PadList } from "../rail/pads/padList";
import { USER } from "./owner";
import { cutName, wholeWord } from "./nameLine";

import type { PadEditorHandle } from "./PadRichEditor";
import "./styles.css";

const PadRichEditor = lazy(() => import("./PadRichEditor"));

const PadBody = (props: {
  initialFocus: ReturnType<typeof captureFocus>;
  initial: Pad;
  revision?: string;
  list?: PadList;
  pads: Accessor<readonly Pad[]>;
  /** T27: a Todo's section link opens the Pad scrolled to this heading. */
  heading?: string | undefined;
  onLateFailure: (message: string) => void;
  onFailureCleared?: (() => void) | undefined;
  /** Successful pad.write reply for the captured original incarnation, even after closure; never an event echo or read recovery. */
  onAcknowledged?: ((incarnation: Pad["incarnation"]) => void) | undefined;
}) => {
  const connected = useConnectedProject();
  const list = props.list ?? usePadList();
  const name = props.initial.name;
  const destination = { surface: "pads", object: props.initial.incarnation, recipient: null };
  const retained = connected.drafts.list().filter(draft => draft.destination.surface === "pads" && draft.destination.object === destination.object && (props.revision === undefined || draft.revision === props.revision)).at(-1);

  const parsed = retained ? v.safeParse(v.pipe(
    v.object({
      originalName: v.string(), source: v.string(), base: v.string(),
      baseRevision: v.pipe(v.number(), v.integer(), v.minValue(1)),
      editorSource: v.optional(v.string()), document: v.optional(v.string()), editorCaret: v.optional(v.boolean()),
    }),
    v.check(value => value.document === undefined
      ? value.editorSource === undefined && value.editorCaret === undefined
      : value.editorSource !== undefined && value.editorCaret !== undefined),
  ), retained.value) : null;

  const recovered = parsed?.success ? retained : undefined;
  const source = parsed?.success ? parsed.output.editorSource ?? parsed.output.source : props.initial.text;
  const base = parsed?.success ? parsed.output.base : props.initial.text;
  let sourceRevision = parsed?.success ? parsed.output.baseRevision : props.initial.revision;
  let originalBase = base;
  let originalRevision = sourceRevision;
  let draftRevision = retained?.revision;
  const [invalid, setInvalid] = createSignal(Boolean(parsed && !parsed.success));
  let draftSelection = recovered?.selection ?? null;
  let captured = recovered ? JSON.stringify([recovered.value.source, recovered.selection, recovered.value.document ?? null]) : null;
  const [paused, setPaused] = createSignal(Boolean(retained));

  const retentionProblem = () => {
    const problem = connected.drafts.problem();

    return problem && (!problem.destination || (problem.destination.surface === destination.surface && problem.destination.object === destination.object)) ? problem : invalid() ? { cause: "invalid" } : null;
  };

  const sourceLabel = copy.copyField(copy.draftField("source").toLowerCase());

  const statusLabel = () => {
    const status = copy.draftStatus[connected.drafts.list().filter(draft => draft.destination.surface === destination.surface && draft.destination.object === destination.object && (props.revision === undefined || draft.revision === props.revision)).at(-1)?.status ?? "draft"];

    return props.revision === undefined ? copy.draftIdentity(copy.currentDraft, status) : status;
  };

  let conflictVersion = 0;
  const [pad, setPad] = createSignal(props.initial);
  const [shown, setShown] = createSignal(parsed?.success ? parsed.output.source : source);
  const [origin, setOrigin] = createSignal(base);
  const [editing, setEditing] = createSignal(Boolean(recovered));
  createEffect(() => { if (editing()) onCleanup(connected.drafts.editor(destination)); });
  const [conflictActor, setConflictActor] = createSignal<Actor | "unknown" | null>(recovered && (base !== props.initial.text || sourceRevision !== props.initial.revision) ? "unknown" : null);

  const conflictLabel = createMemo(() => {
    const actor = conflictActor();

    return actor === null ? "" : actor === "unknown" ? copy.padChanged : copy.changedBy + connected.rail.nameOf(actor);
  });

  const actionFailure = createFailure();
  const [deleted, setDeleted] = createSignal(false);
  const [asking, setAsking] = createSignal(false);
  const [removedByMe, setRemovedByMe] = createSignal(false);
  let deleteButton: HTMLButtonElement | undefined;
  let editor: PadEditorHandle | undefined;
  let pendingActor: Actor | null = null;
  let closed = false;
  let sent: string | null = null;
  let due: "autosave" | "keep-mine" | null = null;
  let finalDraft: string | null | undefined;
  const [saved, setSaved] = createSignal(false);
  const saveAfterPause = debounce(() => flush(), 500);

  const capture = () => {
    if (closed || invalid() || (recovered && !editor)) return;
    const text = currentDraft();
    // Markdown can normalize live spaces and empty paragraphs; retain the exact native draft until acknowledgement.
    const snapshot = editor && (text === null || !editor.unchanged()) ? editor.snapshot() : null;

    if (text === null) actionFailure.show(copy.padSourceMappingFailed);

    if ((text === null && !snapshot) || (!editing() && text === origin())) return;
    const selection = editor?.selection() ?? draftSelection;
    draftSelection = selection;
    const source = text ?? snapshot!.text;
    const fingerprint = JSON.stringify([source, selection, snapshot?.document ?? null]);

    if (captured === fingerprint) return;
    captured = fingerprint;
    const value = { originalName: name, source, base: originalBase, baseRevision: originalRevision };
    const complete = snapshot ? { ...value, editorSource: snapshot.source, document: snapshot.document, editorCaret: snapshot.trailingCaret } : value;
    draftRevision = connected.drafts.edit(destination, complete, selection, "ordinary", draftRevision ?? null).draft.revision;
  };

  /** Moves the source and its dirty/clean baseline together so a resolved change cannot remain a conflict. */
  const settle = (next: Pad) => {
    saveAfterPause.clear();
    due = null;
    setSaved(false);
    setEditing(false);
    sourceRevision = next.revision;
    originalRevision = next.revision;
    originalBase = next.text;
    setOrigin(next.text);
    setShown(next.text);
    editor?.setText(next.text);
    setConflictActor(null);
    actionFailure.clear();
  };

  /** The baseline always moves to the last loaded or saved text, even mid-edit, so a later dirty-check stays correct; the field itself only follows while the user is not typing, or their keystrokes would vanish. */
  const adopt = (next: Pad) => {
    setPad(next);
    sourceRevision = next.revision;
    setOrigin(next.text);

    if (!closed && (!editing() || currentDraft() === next.text)) settle(next);
  };

  const sameContent = (left: Pad, right: Pad) =>
    left.revision === right.revision &&
    left.text === right.text &&
    left.owner.kind === right.owner.kind &&
    left.owner.id === right.owner.id;

  const currentDraft = () => finalDraft !== undefined ? finalDraft : editor ? editor.getText() : shown();

  onCleanup(
    connected.events.subscribe((event) => {
      if (event.name === "pad.changed" && event.data.name === name) pendingActor = event.actor;
    }),
  );

  createEffect(
    on(
      props.pads,
      (listed) => {
        const current = listed.find((entry) => entry.name === name);

        setDeleted(current === undefined || current.incarnation !== destination.object);

        const actor = pendingActor;
        pendingActor = null;

        if (!current || current.incarnation !== destination.object) {
          saveAfterPause.clear();
          due = null;
          setSaved(false);
          setConflictActor(null);
          actionFailure.clear();
          props.onFailureCleared?.();

          return;
        }

        if (sameContent(current, pad())) return;

        setPad(current);

        // Its RPC reply owns acknowledgement; an echo must not erase an undo back to the old baseline.
        if (sent !== null && actor?.kind === "user" && current.text === sent) return;

        if (currentDraft() === origin()) {
          settle(current);

          return;
        }

        saveAfterPause.clear();
        due = null;
        setSaved(false);
        conflictVersion += 1;
        setConflictActor(actor ?? conflictActor() ?? "unknown");
      },
      { defer: true },
    ),
  );

  onCleanup(() => {
    // Snapshot before the lazy editor is destroyed; any queued write keeps this Pad's name and RPC connection.
    capture();
    finalDraft = currentDraft();
    closed = true;
    flush();
    const message = actionFailure.message();

    if (!deleted() && message) props.onLateFailure(message);
  });

  /** A call that fails after the Drawer closed has no ✕ line to land on, so it goes to the Shelf's. */
  const act = async (work: () => Promise<void>) => {
    const message = await failureOf(work);

    if (message === null) return;

    if (closed) props.onLateFailure(message);
    else actionFailure.show(message);
  };

  const ownedByUser = () => pad().owner.kind === "user";

  const write = () =>
    act(async () => {
      if (paused() || sent !== null || due === null || deleted() || !ownedByUser() || (conflictActor() && due !== "keep-mine")) return;

      const resolveConflict = due === "keep-mine";
      const version = conflictVersion;
      const expectedRevision = resolveConflict ? pad().revision : sourceRevision;
      const text = finalDraft === undefined ? currentDraft() : finalDraft;
      due = null;

      if (text === null) {
        const message = copy.padSourceMappingFailed;

        if (closed) props.onLateFailure(message);
        else actionFailure.show(message);

        return;
      }

      if (text === origin() && !resolveConflict) return;

      capture();
      const submission = draftRevision ? connected.drafts.submit(draftRevision) : null;

      if (!submission) return;
      sent = text;
      setSaved(false);

      const timeout = setTimeout(() => {
        connected.drafts.settle(submission.draft.revision, "unknown");
        setPaused(true);
        actionFailure.show(copy.draftStatus.unknown);
      }, 15_000);

      try {
        const next = await connected.app.rpc("pad.write", { name, text, expectedRevision, expectedIncarnation: destination.object });
        connected.drafts.settle(submission.draft.revision, "acknowledged");
        actionFailure.clear();
        props.onFailureCleared?.();

        if (draftRevision === submission.draft.revision) {
          draftRevision = undefined;
          captured = null;
        }

        // A newer explicit choice owns its baseline; an older reply cannot undo it.
        if (version === conflictVersion || conflictActor()) {
          // An unresolved external change still needs an explicit choice.
          if (conflictActor() && (!resolveConflict || version !== conflictVersion)) {
            sourceRevision = next.revision;
            setOrigin(next.text);
          } else adopt(next);
        }

        if (resolveConflict && version === conflictVersion) setConflictActor(null);

        setSaved(!closed && !conflictActor() && currentDraft() === next.text);
      } catch (thrown) {
        connected.drafts.settle(submission.draft.revision, thrown instanceof RpcError && thrown.code !== INTERNAL_CODE ? "failed" : "unknown");
        setPaused(!(thrown instanceof RpcError && thrown.code !== INTERNAL_CODE));
        // A refusal keeps the draft; only another edit or blur retries it.
        due = null;

        if (thrown instanceof RpcError && thrown.code === -32003) {
          saveAfterPause.clear();
          conflictVersion += 1;
          setConflictActor("unknown");
          await list.reload();

          const failure = list.failure();

          if (failure) throw new Error(failure);

          return;
        }

        throw thrown;
      } finally {
        clearTimeout(timeout);
        sent = null;
      }

      props.onAcknowledged?.(destination.object);

      if (due) void write();
    });

  const flush = () => {
    saveAfterPause.clear();
    due ??= "autosave";
    void write();
  };

  /** The Agent that owns the Pad, while it is on the Rail (U432). */
  const owner = () => (pad().owner.kind === "agent" ? connected.rail.nodes.find((node) => node.id === pad().owner.id) : undefined);

  /** U432: selects the owner's row, Resuming it first when it has exited (A22); the Drawer stays open. */
  const openOwner = () =>
    act(async () => {
      const node = owner();

      if (!node) return;

      connected.rail.select(node.id);

      if (connected.rail.exitOf(node) !== null) {
        if (!node.can_resume) throw new Error(copy.hasExitedAndCannotBeResumed(connected.rail.nameOf(pad().owner)));

        await connected.app.rpc("agent.resume", { id: node.id });
        await connected.rail.refresh();
      }
    });

  const exportToFile = () =>
    act(async () => {
      const path = await connected.app.chooseSavePath([name, ".md"].join(""));

      if (path !== null) await connected.app.rpc("pad.export", { name, path });
    });

  const remove = (source: ActivationSource) =>
    act(async () => {
      saveAfterPause.clear();
      due = null;
      await connected.app.rpc("pad.delete", { name });
      setDeleted(true);
      actionFailure.clear();
      props.onFailureCleared?.();
      setRemovedByMe(true);
      connected.drawer.close(source);
    });

  const onChange = () => {
    setSaved(false);
    setEditing(true);

    if (due !== "keep-mine") due = null;

    if (sent === null && editor?.unchanged()) {
      capture();
      settle(pad());

      return;
    }

    capture();

    if (!paused() && ownedByUser() && (!conflictActor() || sent !== null) && !deleted()) saveAfterPause();
  };

  const onText = (text: string) => {
    setShown(text);
    capture();
  };

  const onEditorBlur = (next: EventTarget | null) => {
    if (next instanceof Element && next.closest(".pad-rich-editor, [data-pad-dismiss]")) return;

    if (ownedByUser()) flush();
  };

  /** U58: a Pad opened from under its Agent is taken here, by the owner mark, as U18's click does in the Shelf. */
  const takeIt = () =>
    act(async () => {
      adopt(await connected.app.rpc("pad.setOwner", { name, owner: USER }));
    });

  const keepMine = () => {
    if (sent === null && connected.drafts.list().some(draft => draft.destination.surface === "pads" && draft.destination.object === destination.object && (draft.status === "unknown" || draft.status === "pending"))) {
      void resume();

      return;
    }

    setPaused(false);
    saveAfterPause.clear();
    due = "keep-mine";
    void write();
  };

  const resume = () => act(async () => {
    const pending = connected.drafts.list().filter(draft => draft.destination.surface === "pads" && draft.destination.object === destination.object && (draft.status === "pending" || draft.status === "unknown"));

    if (pending.length) {
      const current = await connected.app.rpc("pad.read", { name });

      if (current.incarnation !== destination.object) {
        setDeleted(true);

        return;
      }

      // pad.read identifies the original but cannot acknowledge a particular write.
      // Even identical current bytes could be another writer's; retain unknown snapshots.
      actionFailure.show(copy.draftStatus.unknown);

      return;
    }

    setPaused(false);
    flush();
  });

  const useTheirs = () => {
    setPaused(false);

    if (draftRevision) connected.drafts.discard(draftRevision);
    draftRevision = undefined;
    setInvalid(false);
    settle(pad());
  };

  const nameCut = createCutTitle(() => name);

  return (
    <div class="pad-body">
      <div class="pad-head">
        <span class="pad-head-title">
          <span {...nameCut.attrs()} class="pad-head-name" style={cutName}>{name}</span>
          <Show when={!ownedByUser()}>
            <span class="pad-owner light" style={wholeWord}>{copy.ownedBy}{connected.rail.nameOf(pad().owner)}</span>
          </Show>
        </span>
        <Show when={!ownedByUser()}>
          <button data-button="quiet" type="button" class="word pad-make-yours" style={wholeWord} onClick={() => void takeIt()}>{copy.makeYours}</button>
        </Show>
        <Show when={owner()}>
          <Tip text={copy.ownerName(connected.rail.nameOf(pad().owner))}>{(trigger) => <button {...trigger()}
            data-button="icon"
            type="button"
            class="word"
            style={wholeWord}
            aria-label={copy.openOwner}
            onClick={() => void openOwner()}
          >
            <Icon name="talk" />
          </button>}</Tip>
        </Show>
        <IconButton style={wholeWord} aria-label={copy.exportMd} onClick={() => void exportToFile()}>
          <Icon name="export" />
        </IconButton>
        <Show when={!deleted()}>
          <IconButton
            style={wholeWord}
            aria-label={copy.delete}
            ref={(button) => (deleteButton = button)}
            onClick={() => {
              actionFailure.clear();
              setAsking(true);
            }}
          >
            <Icon name="trash" />
          </IconButton>
        </Show>
      </div>
      <NativeAlert app={connected.app} open={asking()}
        question={{ title: copy.padDeleteQuestion(name), message: [pad().owner.kind !== "user" ? copy.padDeleteOwnership(connected.rail.nameOf(pad().owner)) : "", currentDraft() !== origin() ? copy.padDeleteUnsaved : ""].filter(Boolean).join("\n"), okLabel: copy.delete, cancelLabel: copy.keep }}
        onCancel={() => setAsking(false)} finalFocus={() => deleteButton ?? null}
        onConfirm={() => { setAsking(false); void remove("key"); }} />
      <ReservedLine data-state={saved() && !deleted() && !conflictActor() ? "saved" : undefined} message={actionFailure.message() ?? (deleted() && !removedByMe() ? `Pad ${name} was deleted` : retentionProblem() ? copy.draftRetention(connected.project.name, copy.pad, retentionProblem()!.cause) : conflictActor() ? conflictLabel() : paused() ? statusLabel() : null)}
        status={saved() && !deleted() && !conflictActor() ? copy.padSaved : null}>
        <Show when={retentionProblem()}>
          <button data-button="primary" type="button" onClick={() => connected.drafts.retry()}>{copy.storageRetry}</button>
          <button data-button="quiet" type="button" onClick={() => void writeText(connected.app, invalid() ? JSON.stringify(retained?.value) : currentDraft() ?? shown()).then(() => actionFailure.clear()).catch(error => actionFailure.show(copy.copyDraftFailure(String(error))))}>{sourceLabel}</button>
          <button data-button="quiet" type="button" onClick={useTheirs}>{copy.discardRevision}</button>
        </Show>
        <Show when={paused() && !retentionProblem()}><button data-button="primary" type="button" disabled={deleted() || invalid()} onClick={() => void resume()}>{copy.resume}</button></Show>
        <Show when={actionFailure.message()}><button data-pad-dismiss data-button="quiet" type="button" onClick={() => { actionFailure.clear(); props.onFailureCleared?.(); }}>{copy.dismiss}</button></Show>
        <Show when={conflictActor()}>
          <button data-button="quiet" type="button" class="word" onClick={() => void keepMine()}>{copy.keepMine}</button>
          <button data-button="quiet" type="button" class="word" onClick={useTheirs}>{copy.useTheirs}</button>
        </Show>
      </ReservedLine>
      <Suspense>
        <PadRichEditor
          initialFocus={props.initialFocus}
          text={source}
          readonly={!ownedByUser() || invalid()}
          onChange={onChange}
          onText={onText}
          onBlur={onEditorBlur}
          onSelection={capture}
          onReady={(ready) => {
            editor = ready;
            queueMicrotask(() => { if (props.heading !== undefined) ready.scrollToHeading(props.heading); });

            // The editor loads lazily; text adopted before it was ready must still reach it.
            if (!editing()) ready.setText(origin());

            if (parsed?.success && parsed.output.document !== undefined && !ready.restoreDocument(parsed.output.document, parsed.output.editorCaret ?? false)) {
              setInvalid(true);
              actionFailure.show(copy.draftRetention(connected.project.name, copy.pad, "invalid"));
            }

            if (recovered?.selection) ready.restoreSelection(recovered.selection);
            else if (recovered) {
              // An uncaptured caret acquires the editor's initial position without creating a revision.
              draftSelection = ready.selection();
              captured = JSON.stringify([recovered.value.source, draftSelection, recovered.value.document ?? null]);
            }
          }}
        />
      </Suspense>

    </div>
  );
};

/** Body of a Pad's Drawer: the Pad is read once, as the user's Touch, after the history that last-touch line shows. */
export const PadDrawer = (props: {
  name: string;
  heading?: string | undefined;
  /** Select a retained revision in this Project, using pads' already-loaded identity facts.
   * Missing revisions or originals render no editor; recovery never performs opening reads.
   */
  recovery?: { project: string; revision: string; list: PadList };
  pads: Accessor<readonly Pad[]>;
  onLateFailure: (message: string) => void;
  onFailureCleared?: (() => void) | undefined;
  /** Successful pad.write reply for the captured original incarnation, even after closure; never an event echo or read recovery. */
  onAcknowledged?: ((incarnation: Pad["incarnation"]) => void) | undefined;
}) => {
  const connected = useConnectedProject();
  const initialFocus = captureFocus();

  if (props.recovery !== undefined) {
    const recovery = props.recovery;

    if (recovery.project !== connected.project.path) return null;

    const retained = connected.drafts.list().find(draft => draft.revision === recovery.revision && draft.destination.surface === "pads" && draft.destination.recipient === null);
    const original = recovery.list.loaded() && recovery.list.failure() === null ? recovery.list.pads().find(pad => pad.name === props.name && pad.incarnation === retained?.destination.object && retained?.value.originalName === pad.name) : undefined;

    return original ? <PadBody initialFocus={initialFocus} initial={original} revision={recovery.revision} list={recovery.list} pads={recovery.list.pads} onLateFailure={props.onLateFailure} onFailureCleared={props.onFailureCleared} onAcknowledged={props.onAcknowledged} /> : null;
  }

  return (
    <ItemDrawer
      item={`pad:${props.name}`}
      read={() => connected.app.rpc("pad.read", { name: props.name })}
      sentence
      touchedBy={(event) => (event.name === "pad.changed" || event.name === "pad.deleted") && event.data.name === props.name}
    >
      {(pad) => (
        <PadBody
          initialFocus={initialFocus}
          initial={pad}
          pads={props.pads}
          heading={props.heading}
          onLateFailure={props.onLateFailure}
          onFailureCleared={props.onFailureCleared}
          onAcknowledged={props.onAcknowledged}
        />
      )}
    </ItemDrawer>
  );
};
