import { captureFocus, queueFocus } from "../keys/focus";
import { writeText } from "../app/clipboard";
import { ExitButton } from "../ui/IconButton";
import { createEffect, createMemo, createResource, createSignal, For, onCleanup, Show, type Accessor } from "solid-js";
import * as v from "valibot";
import type { Message } from "@contracts/message/Message";
import type { Reaction } from "@contracts/reaction/Reaction";
import type { Preview } from "@contracts/reaction/Preview";
import type { Condition } from "@contracts/reaction/Condition";
import { copy } from "../copy";
import { RpcError } from "../app/seam";
import { ReservedLine } from "../ink/ReservedLine";
import { useConnectedProject } from "../state/connectedProject";
import { Checkbox } from "../ui/Checkbox";
import { Select } from "../ui/Select";
import "./styles.css";

const destination = { surface: "reactions", object: "create", recipient: null };

const formSchema = v.object({
  operation: v.string(), name: v.string(), target: v.string(), instruction: v.string(), kind: v.string(),
  ids: v.array(v.string()), any: v.boolean(), repository: v.string(), number: v.string(),
  atDate: v.string(), atTime: v.string(), atOffset: v.string(),
  expDate: v.string(), expTime: v.string(), expOffset: v.string(),
  maxFirings: v.string(), rearm: v.boolean(),
});

type Form = v.InferOutput<typeof formSchema>;

const blank = (operation: string): Form => ({
  operation, name: "", target: "", instruction: "", kind: "agents", ids: [], any: false, repository: "", number: "",
  atDate: "", atTime: "", atOffset: copy.reactionUtc, expDate: "", expTime: "", expOffset: copy.reactionUtc, maxFirings: "1", rearm: false,
});

type View = { kind: "list" } | { kind: "create" } | { kind: "detail"; id: string };

const useNames = () => {
  const { rail, project } = useConnectedProject();

  return (id: string) => {
    const node = rail.nodes.find((node) => node.id === id);

    if (!node) return copy.reactionGone();

    return rail.nodes.some(other => other.name === node.name && other.parent !== node.parent)
      ? copy.draftIdentity(node.name, node.parent === null ? project.name : rail.nodes.find(home => home.id === node.parent)?.name ?? copy.unavailableHome)
      : node.name;
  };
};

const evidenceText = (evidence: string): string | null => {
  let value: unknown;

  try { value = JSON.parse(evidence); }
  catch { return copy.reactionUnknownEvidence; }

  const parsed = v.safeParse(v.object({ source: v.picklist(["agents", "pr-checks", "instant"]) }), value);

  return parsed.success ? null : copy.reactionUnknownEvidence;
};

const conditionText = (condition: Condition, nameOf: (id: string) => string): string =>
  condition.kind === "agents" ? copy.reactionAgents(condition.ids.map(nameOf), condition.any === true)
    : condition.kind === "pr-checks" ? copy.reactionPrChecks(condition.repository, condition.number)
      : copy.reactionInstant(condition.at, condition.late);

/** NR9: saved reactions remain readable when their Agents no longer exist. */
export const createReactions = () => {
  const { app, drafts, daemonExit } = useConnectedProject();

  // Project-owned submissions outlive the overview; settlement refreshes its current mount.
  const submissions = createMemo(() => daemonExit() === null ? JSON.stringify(drafts.list()
    .filter(draft => draft.destination.surface === "reactions" && (draft.status === "pending" || draft.status === "unknown"))
    .map(draft => draft.revision)) : false);

  const [read, { refetch, mutate }] = createResource(submissions, async (_key, { value }): Promise<{ list: Reaction[]; failure: string | null }> => {
    try {
      return { list: await app.rpc("reaction.list", null), failure: null };
    } catch (error) {
      if (!(error instanceof Error)) throw error;

      return { list: value?.list ?? [], failure: error.message };
    }
  }, { initialValue: { list: [], failure: null } });

  const refresh = () => { if (daemonExit() === null) void refetch(); };

  window.addEventListener("focus", refresh);
  onCleanup(() => window.removeEventListener("focus", refresh));

  return { read, refresh, refetch, dismiss: () => mutate(value => ({ ...value, failure: null })) };
};

/** NR9: the Project overview's named reactions: a list, a create form that previews, and a detail with stop and run. */
export const Reactions = (props: { state: ReturnType<typeof createReactions> }) => {
  const { app, drafts, daemonExit } = useConnectedProject();
  const [view, setView] = createSignal<View>({ kind: "list" });
  const [actionFailure, setActionFailure] = createSignal<Record<string, string | null>>({});
  const [pending, setPending] = createSignal(false);
  const [made, setMade] = createSignal<Reaction | null>(null);
  let opener = "";

  drafts.validate("reactions", formSchema);
  drafts.registerRecovery("reactions", (draft, facts) => {
    const target = v.safeParse(v.string(), draft.value.target);

    return facts.daemonExit() !== null || facts.rail.failure() ? "unverified" : !target.success || target.output === "" || facts.rail.nodes.some((node) => node.id === target.output) ? "available" : "unavailable";
  });

  const { read, refetch } = props.state;

  const back = () => {
    setView({ kind: "list" });
    queueFocus(() => document.querySelector<HTMLElement>(`[data-reaction-opener="${opener}"]`)?.focus());
  };

  const open = (next: View) => {
    opener = next.kind === "detail" ? next.id : next.kind;
    setView(next);
    queueFocus(() => document.querySelector<HTMLButtonElement>(".reaction-back")?.focus());
  };

  const selected = () => {
    const current = view();

    return current.kind === "detail" ? current.id : null;
  };

  const [inspection, { refetch: inspect, mutate: inspectValue }] = createResource(selected, async (id): Promise<{ id: string; value: Reaction | null; failure: string | null }> => {
    try {
      return { id, value: await app.rpc("reaction.get", { id }), failure: null };
    } catch (error) {
      if (!(error instanceof Error)) throw error;

      return { id, value: null, failure: error.message };
    }
  });

  const detail = createMemo(() => {
    const current = view();

    if (current.kind !== "detail") return undefined;

    return (inspection()?.value?.id === current.id ? inspection()?.value : undefined) ?? (made()?.id === current.id ? made()! : undefined) ?? read().list.find((reaction) => reaction.id === current.id);
  });

  const act = async (reaction: Reaction, method: "reaction.stop" | "reaction.run") => {
    if (pending()) return;
    setPending(true);

    try {
      setMade(method === "reaction.stop" ? await app.rpc(method, { id: reaction.id }) : await app.rpc(method, { id: reaction.id, operation: crypto.randomUUID() }));
      setActionFailure(value => ({ ...value, [reaction.id]: null }));
    } catch (error) {
      if (!(error instanceof Error)) throw error;
      setActionFailure(value => ({ ...value, [reaction.id]: error.message }));
    }

    if (daemonExit() === null) {
      await refetch();
      await inspect();
    }

    setPending(false);
  };

  return <section class="reactions" aria-label={copy.reactions}>
    <Show when={view().kind === "create"}>
      <ReactionForm onDone={(created, reveal) => {
        setMade(created);

        if (reveal) {
          setView({ kind: "detail", id: created.id });
          queueFocus(() => document.querySelector<HTMLButtonElement>(".reaction-back")?.focus());
        }
      }} onCancel={back} />
    </Show>
    <Show when={view().kind === "detail"}>
      <Show when={detail()} fallback={<ReservedLine message={copy.reactionMissing}><ExitButton onClick={back} /></ReservedLine>}>
        {(reaction) => <div class="reaction-detail" role="region" aria-label={copy.reactionDetail(reaction().name)}>
          <ExitButton class="reaction-back" onClick={back} />
          <h3 class="reaction-title">{reaction().name}</h3>
          <ReservedLine message={(inspection()?.id === selected() ? inspection()?.failure : null) ?? actionFailure()[reaction().id]}>
            <button type="button" data-button="quiet" onClick={() => {
              if (inspection()?.id === selected() && inspection()?.failure) inspectValue(value => value ? { ...value, failure: null } : value);
              else setActionFailure(value => ({ ...value, [reaction().id]: null }));
            }}>{copy.clear}</button>
          </ReservedLine>
          <Facts reaction={reaction()} />
          <div class="reaction-actions">
            <button type="button" data-button="quiet" onClick={() => { void inspect(); }}>{copy.refresh}</button>
            <button type="button" data-button="quiet" disabled={pending() || reaction().state !== "active"} onClick={() => void act(reaction(), "reaction.stop")}>{copy.reactionStop}</button>
            <button type="button" data-button="quiet" disabled={pending() || reaction().state !== "active" || reaction().remaining < 1} onClick={() => void act(reaction(), "reaction.run")}>{copy.reactionRun}</button>
          </div>
        </div>}
      </Show>
    </Show>
    <Show when={view().kind === "list"}>
      <div class="reaction-head">
        <h2 class="overview-heading">{copy.reactions}</h2>
        <button type="button" data-button="quiet" data-reaction-opener="create" onClick={() => open({ kind: "create" })}>{copy.reactionCreate}</button>
      </div>
      <ReservedLine overlay message={read().failure}>
        <button type="button" data-button="quiet" onClick={() => void refetch()}>{copy.retry}</button>
        <button type="button" data-button="quiet" onClick={props.state.dismiss}>{copy.clear}</button>
      </ReservedLine>
      <Show when={read().list.length > 0} fallback={<p class="reaction-empty">{copy.reactionsEmpty}</p>}>
        <ul class="reaction-list">
          <For each={read().list.map(reaction => reaction.id)}>{(id) => {
            const reaction = () => read().list.find(reaction => reaction.id === id)!;

            return <li>
            <button type="button" data-button="quiet" class="reaction-row overview-entry" data-reaction-opener={id} onClick={() => open({ kind: "detail", id })}>
              <span class="reaction-name">{reaction().name}</span>
              <span class="reaction-state" data-state={reaction().state}>{reaction().state === "active" && reaction().observation.evidence && evidenceText(reaction().observation.evidence!) ? copy.reactionUnknownState : copy.reactionState(reaction().state, reaction().observation.health)}</span>
            </button>
          </li>;
          }}</For>
        </ul>
      </Show>
    </Show>
  </section>;
};

const Facts = (props: { reaction: Reaction }) => {
  const nameOf = useNames();
  const { app, rail } = useConnectedProject();
  const [copyFailures, setCopyFailures] = createSignal<Record<string, string | null>>({});
  const r = () => props.reaction;
  const [messageId, setMessageId] = createSignal<number | null>(null);

  const copyOwner = () => messageId() === null ? `reaction:${r().id}` : `message:${messageId()}`;
  const failCopy = (owner: string, message: string | null) => setCopyFailures(value => ({ ...value, [owner]: message }));

  const [message, { refetch, mutate }] = createResource(messageId, async (id): Promise<{ id: number; value: Message | null; failure: string | null }> => {
    try {
      return { id, value: await app.rpc("message.get", { id }), failure: null };
    } catch (error) {
      if (!(error instanceof Error)) throw error;

      return { id, value: null, failure: error.message };
    }
  });

  let returnId: number | null = null;

  const closeMessage = () => {
    setMessageId(null);
    queueFocus(() => document.querySelector<HTMLButtonElement>(`[data-firing-message="${returnId}"]`)?.focus());
  };

  const home = () => r().home;

  const observation = () => r().state !== "active" ? copy.reactionState(r().state, r().observation.health)
    : (r().observation.evidence && evidenceText(r().observation.evidence!)) || copy.reactionHealth(r().observation.health);

  const authority = () => copy.reactionActor(r().actor.kind, rail.nodes.some(node => node.id === r().actor.id) ? nameOf(r().actor.id) : null);

  return <Show when={messageId() !== null} fallback={<dl class="reaction-facts">
    <dt>{copy.reactionAuthority}</dt><dd>{authority()}</dd>
    <dt>{copy.reactionHome}</dt><dd>{home() ? nameOf(home()!) : copy.reactionNoHome}</dd>
    <dt>{copy.reactionTarget}</dt><dd>{nameOf(r().target)}</dd>
    <dt>{copy.reactionInstruction}</dt><dd class="reaction-instruction">{r().instruction}</dd>
    <dt>{copy.reactionCondition}</dt><dd>{conditionText(r().condition, nameOf)}</dd>
    <dt>{copy.reactionBound}</dt><dd>{copy.reactionRemaining(r().remaining, r().max_firings, r().rearm)}</dd>
    <dt>{copy.reactionExpires}</dt><dd><time datetime={r().expires_at}>{copy.reactionWhen(r().expires_at)}</time></dd>
    <dt>{copy.reactionObservation}</dt>
    <dd>
      <span>{observation()}</span>
      <ReservedLine message={r().observation.cause ? copy.reactionCause(r().observation.cause!) : r().observation.gap ? copy.reactionGap(r().observation.gap!) : null} status={!r().observation.cause?.includes("unsupported") && r().observation.recovery ? copy.reactionRecovery(r().observation.recovery!) : null} />
    </dd>
    <dt>{copy.reactionDetails}</dt><dd><button type="button" data-button="quiet" onClick={() => {
      const owner = copyOwner();
      void writeText(app, JSON.stringify({
        name: r().name, authority: authority(),
        target: nameOf(r().target), home: home() ? nameOf(home()!) : copy.reactionNoHome,
        condition: conditionText(r().condition, nameOf), observation: observation(), reaction: r(),
      }, null, 2)).then(() => failCopy(owner, null), error => failCopy(owner, copy.copyDraftFailure(error instanceof Error ? error.message : String(error))));
    }}>{copy.copy}</button><ReservedLine message={copyFailures()[copyOwner()]}><button type="button" data-button="quiet" onClick={() => failCopy(copyOwner(), null)}>{copy.clear}</button></ReservedLine></dd>
    <dt>{copy.reactionFirings}</dt>
    <dd><Show when={r().firings.length > 0} fallback={copy.reactionNoFirings}>
      <ol class="reaction-firings"><For each={r().firings}>{(firing) => <li data-outcome={firing.outcome}><button type="button" data-button="quiet" data-firing-message={firing.message} onClick={() => { returnId = firing.message; setMessageId(firing.message); }}>{copy.reactionFiring(firing.ordinal, firing.message, firing.outcome, firing.matched)}</button></li>}</For></ol>
    </Show></dd>
  </dl>}>
    <section aria-label={copy.reactionMessage(messageId()!)}>
      <ExitButton ref={element => queueFocus(() => element.focus())} onClick={closeMessage} />
      <ReservedLine message={message()?.id === messageId() ? message()?.failure : null}><button type="button" data-button="quiet" onClick={() => void refetch()}>{copy.retry}</button><button type="button" data-button="quiet" onClick={() => mutate(value => value ? { ...value, failure: null } : value)}>{copy.clear}</button></ReservedLine>
      <Show when={!message.loading && message()?.id === messageId() && message()?.value}>{value => <dl class="reaction-facts">
        <dt>{copy.reactionTarget}</dt><dd>{nameOf(value().to)}</dd>
        <dt>{copy.reactionMessage(messageId()!)}</dt><dd class="reaction-instruction">{value().body}</dd>
        <dt>{copy.reactionMessageState}</dt><dd data-state={value().status}>{copy.reactionDelivery(value().status)}{value().reason ? ` · ${copy.reactionCause(value().reason!)}` : ""}</dd>
        <dt>{copy.reactionDetails}</dt><dd><button type="button" data-button="quiet" onClick={() => {
          const owner = copyOwner();
          void writeText(app, JSON.stringify({ target: nameOf(value().to), message: value() }, null, 2))
            .then(() => failCopy(owner, null), error => failCopy(owner, copy.copyDraftFailure(error instanceof Error ? error.message : String(error))));
        }}>{copy.copy}</button><ReservedLine message={copyFailures()[copyOwner()]}><button type="button" data-button="quiet" onClick={() => failCopy(copyOwner(), null)}>{copy.clear}</button></ReservedLine></dd>
      </dl>}</Show>
    </section>
  </Show>;
};

type Resolved = { preview: Preview | null; error: string | null };

const createPreview = (date: Accessor<string>, time: Accessor<string>, offset: Accessor<string>) => {
  const { app } = useConnectedProject();
  const key = createMemo(() => date() && time() && offset() ? { date: date(), time: time(), offset: offset() } : null);

  const [read] = createResource(key, async (params): Promise<Resolved> => {
    try {
      return { preview: await app.rpc("reaction.preview", params), error: null };
    } catch (error) {
      if (!(error instanceof Error)) throw error;

      return { preview: null, error: error.message };
    }
  });

  return createMemo(() => key() === null || read.loading ? undefined : read());
};

const ReactionForm = (props: { onDone: (made: Reaction, reveal: boolean) => void; onCancel: () => void }) => {
  const { app, rail, drafts, project } = useConnectedProject();
  const records = () => drafts.list().filter((draft) => draft.destination.surface === destination.surface && draft.destination.object === destination.object);

  const current = () => records().at(-1);
  const unresolved = () => records().find(draft => draft.status === "pending" || draft.status === "unknown");

  const fresh = blank(crypto.randomUUID());

  const form = (): Form => {
    const parsed = v.safeParse(formSchema, current()?.value);

    return parsed.success ? parsed.output : fresh;
  };

  const set = (patch: Partial<Form>) => {
    const last = current();
    // A submitted or refused payload owns its operation id; a changed payload needs a new one.
    const operation = last && last.status !== "draft" ? crypto.randomUUID() : form().operation;

    drafts.edit(destination, { ...form(), ...patch, operation }, null, "ordinary");
  };

  createEffect(() => { onCleanup(drafts.editor(destination)); });

  const [failure, setFailure] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);
  let disposed = false;

  onCleanup(() => { disposed = true; });
  const agents = () => rail.nodes.filter((node) => node.kind === "agent" || (node.kind === "workstream" && node.status !== null));
  const nameOf = useNames();
  const instant = createPreview(() => form().atDate, () => form().atTime, () => form().atOffset);
  const expiry = createPreview(() => form().expDate, () => form().expTime, () => form().expOffset);

  const resolvedAt = (read: typeof instant): string | null => {
    const resolved = read();

    return resolved?.preview?.at ?? null;
  };

  const condition = (): Condition | null => {
    const f = form();

    if (f.kind === "agents") return f.ids.length > 0 ? { kind: "agents", ids: f.ids, any: f.any } : null;

    if (f.kind === "pr-checks") return f.repository && /^\d+$/.test(f.number) ? { kind: "pr-checks", repository: f.repository, number: Number(f.number) } : null;

    const at = resolvedAt(instant);

    return at ? { kind: "instant", at, late: "once-before-expiry" } : null;
  };

  const ready = () => {
    const f = form();

    return Number.isSafeInteger(Number(f.maxFirings)) && Number(f.maxFirings) > 0 && f.name.trim() !== "" && f.target !== "" && f.instruction.trim() !== "" && condition() !== null && resolvedAt(expiry) !== null && !busy() && !unresolved();
  };

  const home = () => rail.nodes.find(node => node.id === form().target)?.parent ?? null;

  const submit = async () => {
    const draft = current();
    const cond = condition();
    const until = resolvedAt(expiry);
    const f = form();

    if (!draft || !cond || !until || unresolved() || !drafts.submit(draft.revision)) return;

    await send(draft.revision, f, cond, until);
  };

  const send = async (revision: string, f: Form, cond: Condition, until: string, focus = captureFocus()) => {
    setFailure(null);
    setBusy(true);

    try {
      const made = await app.rpc("reaction.create", {
        operation: f.operation, name: f.name, target: f.target, instruction: f.instruction, condition: cond,
        max_firings: Number(f.maxFirings), rearm: f.rearm, expires_at: until,
      });

      drafts.settle(revision, "acknowledged");

      if (!focus(() => props.onDone(made, !disposed && !current()))) props.onDone(made, false);
    } catch (error) {
      if (!(error instanceof Error)) throw error;
      const refused = error instanceof RpcError && [-32700, -32601, -32602, -32001, -32002, -32003].includes(error.code);
      drafts.settle(revision, refused ? "failed" : "unknown");
      setFailure(refused ? copy.reactionNotCreated(error.message) : copy.reactionCreationUnknown(error.message));
    } finally {
      setBusy(false);
    }
  };

  const reconcile = async () => {
    const draft = unresolved();

    if (!draft || draft.status !== "unknown" || busy()) return;

    const focus = captureFocus();
    const f = v.parse(formSchema, draft.value);
    setBusy(true);
    setFailure(null);

    try {
      const until = await app.rpc("reaction.preview", { date: f.expDate, time: f.expTime, offset: f.expOffset });

      const cond: Condition = f.kind === "agents" ? { kind: "agents", ids: f.ids, any: f.any }
        : f.kind === "pr-checks" ? { kind: "pr-checks", repository: f.repository, number: Number(f.number) }
          : { kind: "instant", at: (await app.rpc("reaction.preview", { date: f.atDate, time: f.atTime, offset: f.atOffset })).at, late: "once-before-expiry" };

      await send(draft.revision, f, cond, until.at, focus);
    } catch (error) {
      if (!(error instanceof Error)) throw error;
      setFailure(copy.reactionCreationUnknown(error.message));
    } finally {
      setBusy(false);
    }
  };

  const text = (label: string, key: keyof Form, options: { area?: boolean; placeholder?: string } = {}) => <label class="reaction-field">
    <span class="reaction-label">{label}</span>
    {options.area
      ? <textarea rows={3} value={String(form()[key])} onInput={(event) => set({ [key]: event.currentTarget.value })} />
      : <input type="text" value={String(form()[key])} placeholder={options.placeholder} onInput={(event) => set({ [key]: event.currentTarget.value })} />}
  </label>;

  const when = (label: string, date: "atDate" | "expDate", time: "atTime" | "expTime", offset: "atOffset" | "expOffset", read: typeof instant) => {
    const dateField = text(copy.reactionDate, date, { placeholder: copy.reactionDatePlaceholder });
    const timeField = text(copy.reactionTime, time, { placeholder: copy.reactionTimePlaceholder });
    const offsetField = text(copy.reactionOffset, offset, { placeholder: copy.reactionOffsetPlaceholder });

    return <div class="reaction-when" role="group" aria-label={label}>
    <span class="reaction-label">{label}</span>
    {dateField}
    {timeField}
    {offsetField}
    <ReservedLine message={read()?.error} status={read()?.preview ? copy.reactionResolved(read()!.preview!) : null} />
  </div>;
  };

  const nameField = text(copy.reactionName, "name");
  const instructionField = text(copy.reactionInstruction, "instruction", { area: true });
  const repositoryField = text(copy.reactionRepository, "repository", { placeholder: copy.reactionRepositoryPlaceholder });
  const numberField = text(copy.reactionPr, "number");
  const maxField = text(copy.reactionMax, "maxFirings");
  const atFields = when(copy.reactionAt, "atDate", "atTime", "atOffset", instant);
  const expiryFields = when(copy.reactionExpires, "expDate", "expTime", "expOffset", expiry);

  return <form class="reaction-form" aria-label={copy.reactionCreate} onSubmit={(event) => { event.preventDefault(); void submit(); }}>
    <ExitButton class="reaction-back" onClick={props.onCancel} />
    <h3 class="reaction-title">{copy.reactionCreate}</h3>
    {nameField}
    <Select label={copy.reactionTarget} value={form().target}
      choices={[{ value: "", label: copy.reactionPickTarget }, ...agents().map((node) => ({ value: node.id, label: nameOf(node.id) })), ...(form().target && !agents().some(node => node.id === form().target) ? [{ value: form().target, label: nameOf(form().target) }] : [])]}
      onValueChange={(target) => set({ target })} />
    {instructionField}
    <Select label={copy.reactionCondition} value={form().kind}
      choices={[{ value: "agents", label: copy.reactionKindAgents }, { value: "pr-checks", label: copy.reactionKindPr }, { value: "instant", label: copy.reactionKindInstant }]}
      onValueChange={(kind) => set({ kind })} />
    <Show when={form().kind === "agents"}>
      <div class="reaction-agents" role="group" aria-label={copy.reactionWatched}>
        <span class="reaction-label">{copy.reactionWatched}</span>
        <For each={agents()}>{(node) => <Checkbox checked={form().ids.includes(node.id)}
          onCheckedChange={(checked) => set({ ids: checked ? [...form().ids, node.id] : form().ids.filter((id) => id !== node.id) })}>{nameOf(node.id)}</Checkbox>}</For>
        <Checkbox checked={form().any} onCheckedChange={(any) => set({ any })}>{copy.reactionAny}</Checkbox>
      </div>
    </Show>
    <Show when={form().kind === "pr-checks"}>{repositoryField}{numberField}</Show>
    <Show when={form().kind === "instant"}>{atFields}</Show>
    {expiryFields}
    {maxField}
    <Checkbox checked={form().rearm} onCheckedChange={(rearm) => set({ rearm })}>{copy.reactionRearm}</Checkbox>
    <section class="reaction-preview" aria-label={copy.reactionPreview}>
      <span class="reaction-label">{copy.reactionPreview}</span>
      <dl class="reaction-facts">
        <dt>{copy.reactionAuthority}</dt><dd>{copy.reactionPreviewActor}</dd>
        <dt>{copy.reactionHome}</dt><dd>{home() ? nameOf(home()!) : copy.reactionNoHome}</dd>
        <dt>{copy.reactionTarget}</dt><dd>{form().target ? nameOf(form().target) : copy.reactionPickTarget}</dd>
        <dt>{copy.reactionInstruction}</dt><dd class="reaction-instruction">{form().instruction}</dd>
        <dt>{copy.reactionCondition}</dt><dd>{condition() ? conditionText(condition()!, nameOf) : copy.reactionIncomplete}</dd>
        <dt>{copy.reactionBound}</dt><dd>{copy.reactionRemaining(Number(form().maxFirings) || 1, Number(form().maxFirings) || 1, form().rearm)}</dd>
        <dt>{copy.reactionExpires}</dt><dd>{resolvedAt(expiry) ?? copy.reactionIncomplete}</dd>
      </dl>
    </section>
    <ReservedLine message={drafts.problem() ? copy.draftRetention(project.name, copy.draftSurface("reactions"), drafts.problem()!.cause) : unresolved()?.status === "unknown" ? failure() ?? copy.reactionCreationUnknown("") : failure()}>
      <Show when={drafts.problem()}><button type="button" data-button="quiet" onClick={() => drafts.retry()}>{copy.storageRetry}</button></Show>
      <Show when={failure() && !drafts.problem() && !unresolved()}><button type="button" data-button="quiet" onClick={() => setFailure(null)}>{copy.clear}</button></Show>
      <Show when={unresolved()?.status === "unknown"}><button type="button" data-button="quiet" disabled={busy()} onClick={() => void reconcile()}>{copy.reactionCheckCreation}</button></Show>
    </ReservedLine>
    <div class="reaction-actions">
      <button type="submit" data-button="quiet" disabled={!ready()}>{copy.reactionSave}</button>
    </div>
  </form>;
};
