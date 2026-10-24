import { captureFocus } from "../keys/focus";
import { createCutTitle } from "../ui/cutTitle";
import { createEffect, createMemo, createSignal, on, onCleanup, Show } from "solid-js";
import type { Accessor } from "solid-js";
import type { ChangedFiles } from "@contracts/agent/ChangedFiles";
import type { Worktree } from "@contracts/agent/Worktree";
import { copy } from "../copy";
import { ReservedLine } from "../ink/ReservedLine";
import { Icon } from "../ink/Icon";
import { failureOf } from "../todos/failureOf";
import { useConnectedProject } from "../state/connectedProject";
import { Popover } from "../ui/Popover";
import { notesFor } from "./notes";
import type { Notes, Note } from "./notes";
import { NoteBatch, NoteRecovery } from "./NoteBatch";
import { DiffReader } from "./DiffReader";
import { ChangedTree } from "./ChangedTree";
import "./styles.css";

type Read = { failure: null; changed: ChangedFiles } | { failure: string } | null;

/** U2539b: the panel of one Agent's Worktree. It reads again on every open and closes when the selection moves, because it is keyed on the Agent. */
const Panel = (props: { id: string; name: string; worktree: Worktree; notes: Notes; recoveryOpen: boolean; onOpen: () => void; recover: () => void }) => {
  const { app, worktrees } = useConnectedProject();
  const state = () => worktrees.read(props.id);

  const facts = () => {
    const read = state();

    return read?.failure === null ? read.state : null;
  };

  let treeFocus = captureFocus();
  const [open, setOpen] = createSignal(false);
  const [read, setRead] = createSignal<Read>(null);
  const [version, setVersion] = createSignal(0);
  const [dismissedState, setDismissedState] = createSignal<ReturnType<typeof state>>();
  const [dismissedRead, setDismissedRead] = createSignal<Read>();

  const failure = () => {
    const shared = state();
    const local = read();

    return (shared !== dismissedState() ? shared?.failure : null) ?? (local !== dismissedRead() ? local?.failure : null);
  };

  const dismiss = () => {
    const shared = state();

    if (shared?.failure && shared !== dismissedState()) setDismissedState(shared);
    else setDismissedRead(read());
  };

  const [path, setPath] = createSignal<string>();
  const [editing, setEditing] = createSignal<Note>();
  const [diff, setDiff] = createSignal(false);
  const [control, setControl] = createSignal<HTMLButtonElement>();

  createEffect(on(() => [open(), version()] as const, ([now]) => {
    if (!now) return;

    let current = true;

    setRead(previous => previous?.failure ? previous : null);

    let loaded: Read = null;

    void failureOf(async () => {
      const [, changed] = await Promise.all([worktrees.refresh(props.id), app.rpc("agent.changedFiles", { id: props.id })]);

      loaded = { failure: null, changed };
    }).then((failure) => {
      if (current) setRead(failure === null ? loaded : { failure });
    });

    onCleanup(() => { current = false; });
  }));

  const hide = () => { setOpen(false); setDiff(false); setEditing(undefined); };

  const branchCut = createCutTitle(() => props.worktree.branch);
  const baseCut = createCutTitle(() => props.worktree.base);

  const close = () => { hide(); control()?.focus(); };

  let viewButton: HTMLButtonElement | undefined;
  const openPanel = () => { treeFocus = captureFocus(true); props.onOpen(); setOpen(true); };

  createEffect(() => { if (props.recoveryOpen) hide(); });

  return (
    <Popover.Root initialFocus={treeFocus} closeOnInteractOutside={false} open={open()} closeOnEscape onOpenChange={({ open: next }) => { if (next) openPanel(); else close(); }} positioning={{ placement: "top-end", strategy: "fixed" }}>
      <Popover.Trigger asChild={(trigger) => (
        <button {...trigger()} ref={setControl} type="button" data-button="quiet" class="worktree-control" aria-label={props.worktree.branch} onClick={() => (open() ? close() : openPanel())}>
          <Icon name="branch" />
          <span class="worktree-branch">{props.worktree.branch}</span>
        </button>
      )} />
      <Popover.Positioner>
        <Popover.Content class="worktree-panel" data-diff={diff() || undefined} aria-label={copy.worktree} onKeyDown={(event) => { if (event.key === "Escape") { event.stopPropagation(); close(); } }}>
          <Show when={!diff()}><div class="diff-actions"><button type="button" data-button="quiet" onClick={close}>{copy.close}</button></div></Show>
          <Show when={!diff()} fallback={<DiffReader id={props.id} name={props.name} path={path()!} notes={props.notes} editing={editing()} onEdited={() => setEditing(undefined)} onBack={() => { setEditing(undefined); setDiff(false); }} onClose={close} recover={props.recover} />}>
          <dl class="worktree-facts">
            <dt>{copy.branch}</dt>
            <dd {...branchCut.attrs()} style={{ width: "fit-content", "max-width": "100%" }}>{props.worktree.branch}</dd>
            <dt>{copy.base}</dt>
            <dd {...baseCut.attrs()} style={{ width: "fit-content", "max-width": "100%" }}>{props.worktree.base}</dd>
            <Show when={facts()}>
              {(state) => (
                <>
                  <dt>{copy.worktreeState}</dt>
                  <dd>{copy.worktreeStateText(state().ahead, state().behind, state().dirty)}</dd>
                  <dt>{copy.worktreeCi}</dt>
                  <dd><Show when={state().ci} fallback={copy.worktreeCiUnavailable}>{(ci) => copy.worktreeCiText(ci().state, ci().stale)}</Show></dd>
                  <dt>{copy.worktreePr}</dt>
                  <dd><Show when={state().pr} fallback={copy.worktreePrUnavailable}>{(pr) => copy.worktreePrText(pr().number, pr().stale)}</Show></dd>
                </>
              )}
            </Show>
          </dl>
          <ReservedLine message={failure()}><button type="button" data-button="quiet" onClick={() => { treeFocus = captureFocus(); treeFocus(() => {}); setVersion(value => value + 1); }}>{copy.retry}</button><button type="button" data-button="quiet" onClick={dismiss}>{copy.dismissFailure}</button></ReservedLine>
          <Show when={read()} keyed>
            {(value) => {

              if (value.failure !== null) return null;

              if (value.changed.files.length === 0) return <p class="worktree-none">{copy.noChanges}</p>;

              return <><ChangedTree initialFocus={treeFocus} root={props.worktree.path} files={value.changed.files} selected={path()} onSelect={setPath} /><button type="button" data-button="quiet" ref={element => { viewButton = element; }} disabled={!path()} onClick={() => setDiff(true)}>{copy.viewDiff}</button></>;
            }}
          </Show>
        <NoteBatch fallback={() => viewButton?.focus()} recover={props.recover} id={props.id} name={props.name} notes={props.notes} edit={note => { setPath(note.comparison.newPath); setEditing(note); setDiff(true); }} />
          </Show>
        </Popover.Content>
      </Popover.Positioner>
    </Popover.Root>
  );
};

/** U2539b: the bottom bar's one control for the selected Agent's Worktree; none for an Agent without one. */
export const WorktreeControl = () => {
  const { rail, app, project, drafts } = useConnectedProject();
  const notes = notesFor(app, project.path, drafts);
  const names = new Map<string, string>();

  createEffect(() => { for (const node of rail.nodes) names.set(node.id, node.name); });

  const [recoveryOpen, setRecoveryOpen] = createSignal(false);

  const selected: Accessor<{ id: string; name: string; worktree: Worktree } | null> = createMemo(
    () => {
      const node = rail.nodes.find((candidate) => candidate.id === rail.selected());

      return node && node.worktree ? { id: node.id, name: node.name, worktree: node.worktree } : null;
    },
    null,
    { equals: (a, b) => a?.id === b?.id && a?.worktree.branch === b?.worktree.branch && a?.worktree.path === b?.worktree.path && a?.worktree.base === b?.worktree.base },
  );

  return <><Show when={selected()} keyed>{(each) => <Panel recover={() => setRecoveryOpen(true)} id={each.id} name={each.name} worktree={each.worktree} notes={notes} recoveryOpen={recoveryOpen()} onOpen={() => setRecoveryOpen(false)} />}</Show><NoteRecovery name={id => rail.nodes.find(node => node.id === id)?.name ?? names.get(id) ?? copy.unavailableAgent} notes={notes} open={recoveryOpen()} onOpenChange={setRecoveryOpen} available={id => rail.nodes.some(node => node.id === id && node.worktree !== null)} /></>;
};
