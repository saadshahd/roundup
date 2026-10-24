import { captureFocus, queueFocus } from "../keys/focus";
import { writeText } from "../app/clipboard";
import { batch, untrack, createResource, createEffect, createMemo, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import * as v from "valibot";
import type { AskSubmission } from "../state/asks";
import { copy } from "../copy";
import { openDeliverable } from "./openDeliverable";
import { Icon } from "../ink/Icon";
import { ReservedLine } from "../ink/ReservedLine";
import { ASK_CLOSE, CHORDS, chordsFor } from "../keys/chords";
import { modalOpen, modalOutside, registerKeys, type KeyScope } from "../keys/registry";
import { useConnectedProject } from "../state/connectedProject";
import { Mentions } from "../ui/Mentions";
import { KindGlyph, LABELS } from "../ink/KindGlyph";
import { attentionOrder } from "../rail/attention";
import { descendantsOf, resolvedWorkstream, siblingsOf } from "../rail/layout";
import { Tip } from "../ui/Tip";
import { contextMenu, linkEntries, openOnRelease } from "../ui/contextMenu";
import type { Deliverable } from "@contracts/deliverable/Deliverable";
import { Popover } from "../ui/Popover";
import { Portal } from "solid-js/web";
import { shelfScope } from "../shelf/scope";
import type { RailNode } from "@contracts/agent/RailNode";
import { AskRows } from "./AskRows";
import { createReactions, Reactions } from "../reactions/Reactions";
import { ToolCallLine } from "./ToolCallLine";
import "./styles.css";

/** The leading `@` and the letters after it, while the user has not yet typed the space that ends the name (U504). */
const mentionOf = (text: string): string | null => /^@(\S*)$/.exec(text)?.[1] ?? null;

/** U504: the Ask bar and its rows. The Project's home screen while the Rail selects no Workstream, and `⌘/` opens it over anything else; Escape closes that and returns focus. */
export const AskBar = (props: {
  /** Change on the mounted owner to open one retained revision without reloading its readers.
   * Closing returns ordinary editing to the current work; selection never submits or reconciles.
   */
  recovery?: { project: string; revision: string } | undefined;
}) => {
  const connected = useConnectedProject();
  const { asks, rail, daemonExit, drafts } = connected;
  const deliverables = createDeliverables();
  const reactions = createReactions();
  const madeScope = shelfScope(connected);
  const [over, setOver] = createSignal(false);
  const [selected, setSelected] = createSignal<string | null>(null);
  const destination = { surface: "ask", object: "ask-bar", recipient: null };
  const formSchema = v.object({ text: v.string(), workstream: v.optional(v.nullable(v.string())), name: v.optional(v.nullable(v.string())), recovered: v.optional(v.literal(true)) });

  drafts.validate("ask", formSchema);
  drafts.registerRecovery("ask", (draft, facts) => {
    const target = v.safeParse(v.string(), draft.value.workstream);
    const available = !target.success || facts.rail.nodes.some(node => node.kind === "workstream" && node.id === target.output);
    const state = facts.daemonExit() !== null || facts.rail.failure() ? "unverified" : available ? "available" : "unavailable";

    return state;
  });
  const records = () => drafts.list().filter(draft => draft.destination.surface === "ask" && draft.destination.object === "ask-bar" && draft.destination.recipient === null);

  const problem = () => {
    const found = drafts.problem();

    const own = found?.destination?.surface === destination.surface
      && found.destination.object === destination.object && found.destination.recipient === destination.recipient;

    return !connected.drawer.content() && own ? found : null;
  };

  // Recovered edits remain selectable without displacing the ordinary current work.
  const ordinary = () => records().filter(draft => draft.value.recovered !== true).at(-1);

  const current = () => {
    if (selected() !== null) return records().find(draft => draft.revision === selected() && draft.status !== "pending");

    const latest = ordinary();

    return latest?.status === "draft" || latest?.status === "failed" ? latest : undefined;
  };

  const text = () => {
    const value = v.safeParse(v.string(), current()?.value.text);

    return value.success ? value.output : "";
  };

  const setText = (value: string, recipient?: string) => {
    const previous = current();
    const captured = v.safeParse(v.string(), previous?.value.workstream);
    const previousName = v.safeParse(v.string(), previous?.value.name);
    const starts = (text: string, name: string) => text.toLowerCase().startsWith(`@${name.toLowerCase()} `);

    const named = recipient === undefined
      ? rail.nodes.filter(node => node.kind === "workstream" && starts(value, node.name)).sort((a, b) => b.name.length - a.name.length)[0]
      : rail.nodes.find(node => node.kind === "workstream" && node.id === recipient);

    // Newly typed longer names are a new choice; a replacement matching unchanged text is not.
    const keep = recipient === undefined && captured.success && previousName.success && starts(value, previousName.output)
      && !(named && named.name.length > previousName.output.length && !starts(text(), named.name));

    const workstream = keep ? captured.output : named?.id ?? null;
    const name = keep ? previousName.output : named?.name ?? null;

    const recovering = selected() !== null;

    const recovered = previous?.value.recovered === true || (recovering && previous?.revision !== ordinary()?.revision);

    const form = { text: value, workstream, name };

    const retained = drafts.edit(destination, recovered ? { ...form, recovered: true } : form,
      field ? { start: field.selectionStart, end: field.selectionEnd, direction: field.selectionDirection } : null, "ordinary",
      previous?.revision ?? null);

    if (recovering) setSelected(retained.draft.revision);
  };

  const retainSelection = () => {
    const draft = current();

    if (!draft || !field?.isConnected) return;
    const selection = { start: field.selectionStart, end: field.selectionEnd, direction: field.selectionDirection };

    if (JSON.stringify(draft.selection) !== JSON.stringify(selection)) drafts.setSelection(draft.revision, selection);
  };

  const restoreSelection = (element: HTMLTextAreaElement) => {
    field = element;
    queueMicrotask(() => {
      const caret = current()?.selection;

      if (element.isConnected && caret) element.setSelectionRange(caret.start, caret.end, caret.direction);
    });
  };

  const [failure, setFailure] = createSignal<{ revision: string; message: string } | null>(null);
  const [dismissed, setDismissed] = createSignal(false);

  const dismissSend = () => {
    setFailure(null);

    const intake = asks.intakeFailure();

    if (intake) asks.dismiss(intake);
  };

  const [highlighted, setHighlighted] = createSignal<string | null>(null);
  let panel: HTMLElement | undefined;
  let field: HTMLTextAreaElement | undefined;
  let returnTo: Element | null = null;

  // With no Workstream at all the Pane's "New Workstream" is the way in, so home leaves it uncovered.
  const shown = () => over() || (rail.selected() === null && (rail.nodes.length > 0 || deliverables.records().length > 0 || deliverables.failure() !== null || reactions.read().list.length > 0 || reactions.read().failure !== null));

  createEffect(() => {
    if (shown()) onCleanup(drafts.editor(destination));
  });

  asks.load();

  const names = createMemo(() => rail.nodes.filter((node) => node.kind === "workstream").map((node) => ({ value: node.name, label: node.name })));

  const matching = createMemo(() => {
    const typed = mentionOf(text());

    return dismissed() || typed === null ? [] : names().filter((item) => item.label.toLowerCase().includes(typed.toLowerCase()));
  });

  createEffect(() => {
    const list = matching();

    if (!list.some((item) => item.value === highlighted())) setHighlighted(list[0]?.value ?? null);
  });

  const close = (restore: boolean): void => {
    batch(() => { setOver(false); setSelected(null); });

    if (field?.isConnected) restoreSelection(field);

    if (restore && returnTo instanceof HTMLElement && returnTo.isConnected) returnTo.focus();
  };

  createEffect(() => { if (over() && modalOutside(panel)) close(false); });

  const open = (): void => {
    if (modalOpen()) return;
    captureFocus(true);
    returnTo = document.activeElement;
    setOver(true);
    queueFocus(() => {
      field?.focus();

      if (field) restoreSelection(field);
    });
  };

  createEffect(() => {
    const recovery = props.recovery;

    untrack(() => {
      if (!recovery || recovery.project !== connected.project.path) return;
      const draft = records().find(entry => entry.revision === recovery.revision);

      if (!draft || draft.status === "pending" || drafts.availability(draft, connected) !== "available") return;
      batch(() => { setSelected(draft.revision); open(); });
    });
  });

  registerKeys({ kind: "app" }, [{
    action: CHORDS.find((row) => row.keys === "⌘/")!,
    when: () => daemonExit() === null && !over(),
    capture: () => ({ available: () => daemonExit() === null && !over(),
    invoke: open }),
  }]);
  const scope: KeyScope = { kind: "layer", element: () => panel, active: over, modal: true };

  registerKeys(scope, [
    { chords: chordsFor("⌘/"), run: (press) => { if (!press.repeat) close(true); } },
    { chords: ASK_CLOSE, run: () => { if (matching().length > 0) setDismissed(true); else close(true); } },
  ]);

  const send = (): void => {
    const value = text();

    if (value.trim() === "") return;

    const draft = current();

    if (!draft || !drafts.submit(draft.revision)) return;
    setFailure(null);
    const workstream = v.safeParse(v.string(), draft.value.workstream);
    const request: AskSubmission = { text: value, submission: draft.revision };

    if (workstream.success) request.workstream = workstream.output;
    void asks.send(request).then((outcome) => {
      drafts.settle(draft.revision, outcome.kind === "refused" ? "failed" : outcome.kind);

      if (outcome.kind === "refused") setFailure({ revision: draft.revision, message: copy.askNotSent + outcome.error.message });
    });
  };

  const complete = (name: string): void => {
    setText(`@${name} `, rail.nodes.find(node => node.kind === "workstream" && node.name === name)?.id);
    field?.focus();
  };

  const onKey = (event: KeyboardEvent): void => {
    if (event.key === "Escape" && matching().length > 0) {
      event.preventDefault();
      event.stopPropagation();
      setDismissed(true);

      return;
    }

    if (event.key !== "Enter" || event.shiftKey || event.isComposing) return;

    event.preventDefault();

    const picked = matching().length > 0 ? highlighted() : null;

    if (picked !== null) complete(picked);
    else send();
  };

  return (<>
    <Show when={rail.selected() !== null && (deliverables.records().some(record => madeScope.includes(record.home, record.maker)) || deliverables.failure() !== null)}><div class="made-context"><Popover.Root closeOnEscape closeOnInteractOutside positioning={{ placement: "top-start", strategy: "fixed" }}>
      <Popover.Trigger asChild={attributes => <button {...attributes()} type="button" data-button="quiet" class="made-trigger">{copy.deliverables}<Icon name="down" /></button>} />
      <Portal><Popover.Positioner><Popover.Content class="made-panel"><Deliverables state={deliverables} /></Popover.Content></Popover.Positioner></Portal>
    </Popover.Root></div></Show>
    <Show when={shown()}>
      <section class="ask-bar" data-mode={over() ? "over" : "home"} aria-label={copy.askBarLabel} ref={(element) => { panel = element; }}>
        <div class="ask-compose">
        <Mentions.Root onDismiss={() => setDismissed(true)} items={matching()} highlightedValue={highlighted()} onHighlightChange={setHighlighted} onSelect={complete}>
          <Mentions.Field class="ask-input" ref={restoreSelection}
            rows={1} aria-label={copy.askBarLabel} placeholder={copy.askBarPlaceholder} value={text()} open={matching().length > 0}
            onInput={(event) => { setDismissed(false); setText(event.currentTarget.value); }} onSelect={retainSelection} onKeyUp={retainSelection} onClick={retainSelection} onKey={onKey} />
          <Show when={matching().length > 0}><Mentions.List label={copy.askMentions} items={matching()} /></Show>
        </Mentions.Root>
        <ReservedLine overlay class="ask-send-status" message={problem() ? copy.draftRetention(connected.project.name, copy.draftSurface("ask"), problem()!.cause) : failure()?.message ?? (records().some(draft => draft.status === "unknown") ? null : asks.failure())}
          status={records().some(draft => draft.status === "unknown") ? copy.askUnknownState : null}>
        <For each={records().filter(draft => draft.status === "unknown").map(draft => draft.revision)}>{revision => {
          const [cause, setCause] = createSignal<string | null>(null);
          const label = () => String(records().find(draft => draft.revision === revision)?.value.text ?? "");

          return <div class="reserved-unknown">
            <div class="ask-unknown-content">
              <span class="ask-unknown-text">{label()}</span>
              <span data-state="unknown">{cause() ? copy.askUnknown + cause() : copy.askUnknownState}</span>
            </div>
            <button type="button" data-button="quiet" aria-label={copy.askCheckAcceptanceLabel(label())} onClick={() => {
              void asks.reconcile(revision).then(outcome => {
                drafts.settle(revision, outcome.kind === "refused" ? "failed" : outcome.kind);

                if (outcome.kind === "unknown") setCause(outcome.message);
                else setFailure(previous => previous?.revision === revision ? null : previous);
              });
            }}>{copy.askCheckAcceptance}</button>
          </div>;
        }}</For>
        <Show when={problem()}><button type="button" data-button="quiet" onClick={() => drafts.retry()}>{copy.storageRetry}</button></Show>
        <Show when={!problem() && !records().some(draft => draft.status === "unknown")}>
          <Show when={failure()} fallback={<Show when={asks.listFailure() ?? asks.intakeFailure()}>{each => <button type="button" data-button="quiet" onClick={() => asks.dismiss(each())}>{copy.dismiss}</button>}</Show>}>
            <button type="button" data-button="quiet" onClick={dismissSend}>{copy.dismiss}</button>
          </Show>
        </Show>
        </ReservedLine>
        </div>
        <Show when={!over()}><ProjectOverview /><Reactions state={reactions} /><Deliverables state={deliverables} /></Show>
        <AskRows keys={scope} onOpened={() => close(false)} />
      </section>
    </Show>
  </>);
};

const ProjectOverview = () => {
  const { rail } = useConnectedProject();
  const attention = createMemo(() => attentionOrder(rail.nodes));
  const workstreams = createMemo(() => siblingsOf(rail.nodes, null).flatMap((node) => [node, ...descendantsOf(rail.nodes, node.id)]).filter((node) => node.kind === "workstream"));
  const under = (id: string | null) => attention().filter((node) => resolvedWorkstream(rail.nodes, node.id) === id);

  const called = (id: string | null) => rail.nodes.filter((node) => node.tool_call !== undefined && resolvedWorkstream(rail.nodes, node.id) === id);

  const entry = (node: RailNode, waiting = false) => <OverviewEntry node={node} waiting={waiting} select={() => rail.select(node.id)} />;

  return <section class="project-overview" aria-label={copy.projectOverview}>
    <ReservedLine overlay message={rail.failure()} />
    <Show when={attention().length > 0} fallback={<div class="empty-state overview-empty">
      <Icon name="check" />
      <h2 class="overview-heading" data-state="attention">{copy.projectAttention(0)}</h2>
    </div>}>
      <h2 class="overview-heading" data-state="attention">{copy.projectAttention(attention().length)}</h2>
    </Show>
    <ul class="overview-list">
      <For each={workstreams()}>{(node) => <li>
        {entry(node)}
        <Show when={under(node.id).length > 0}><ul class="overview-attention">
          <For each={under(node.id)}>{(agent) => <li>{entry(agent, true)}</li>}</For>
        </ul></Show>
        <Show when={called(node.id).length > 0}><ul class="overview-calls">
          <For each={called(node.id)}>{(agent) => <li><ToolCallLine name={agent.name} call={agent.tool_call!} /></li>}</For>
        </ul></Show>
      </li>}</For>
      <For each={under(null)}>{(node) => <li>{entry(node, true)}</li>}</For>
      <For each={called(null)}>{(agent) => <li><ToolCallLine name={agent.name} call={agent.tool_call!} /></li>}</For>
    </ul>
  </section>;
};

const OverviewEntry = (props: { node: RailNode; waiting: boolean; select: () => void }) => {
  const [cut, setCut] = createSignal(false);
  let name: HTMLSpanElement | undefined;
  const measure = () => setCut(!!name && name.scrollWidth > name.clientWidth);

  createEffect(() => {
    if (props.node.name) queueMicrotask(measure);
  });
  onMount(() => {
    if (!name) return;
    const observer = new ResizeObserver(measure);

    observer.observe(name);
    onCleanup(() => observer.disconnect());
  });

  return <Tip text={props.node.name} disabled={!cut()}>{(trigger, focus) => {
    const tip = trigger();

    return <button {...focus} onClick={props.select} type="button" class="overview-entry"
      aria-label={[props.node.name, props.node.status ? LABELS[props.node.status.kind] : ""].filter(Boolean).join(" ")}
      data-feedback="ink" data-button="quiet" onPointerEnter={measure}>
      <Tip text={props.node.status ? LABELS[props.node.status.kind] : ""} disabled={!props.node.status}>{(trigger) =>
        <span {...trigger()} class={props.waiting ? "overview-state ink" : "overview-state"} data-state={props.node.status?.kind} data-tone={props.node.status?.kind === "error" ? "red" : undefined}>
          <Show when={props.node.status}>{(status) => <KindGlyph kind={status().kind} />}</Show>
        </span>
      }</Tip>
      <span {...tip} ref={name} class={props.waiting ? "overview-name ink" : "overview-name"}>{props.node.name}</span>
    </button>;
  }}</Tip>;
};

/** DV5: metadata stays separate from resolution. Never retain a resolved path between opens. */
const createDeliverables = () => {
  const { app, events, daemonExit } = useConnectedProject();

  type Catalogue = { records: Deliverable[]; failure: string | null };

  const [read, { refetch }] = createResource(async (_key, { value }): Promise<Catalogue> => {
    try {
      return { records: await app.rpc("deliverable.list", { home: null, maker: null }), failure: null };
    } catch (error) {
      if (!(error instanceof Error)) throw error;

      return { records: value?.records ?? [], failure: error.message };
    }
  }, { initialValue: { records: [], failure: null } satisfies Catalogue });

  const refresh = () => { if (daemonExit() === null) void refetch(); };

  window.addEventListener("focus", refresh);
  const unsubscribe = events.subscribe(event => { if (event.name === "rail.changed") refresh(); });
  onCleanup(() => { unsubscribe(); window.removeEventListener("focus", refresh); });

  return { records: () => read().records, failure: () => read().failure, loading: () => read.loading, refresh };
};

const Deliverables = (props: { state: ReturnType<typeof createDeliverables> }) => {
  const connected = useConnectedProject();
  const scope = shelfScope(connected);
  const records = createMemo(() => props.state.records().filter(record => scope.includes(record.home, record.maker)));
  const [selected, setSelected] = createSignal<string | null>(null);
  const [pending, setPending] = createSignal<string | null>(null);
  const [failures, setFailures] = createSignal<ReadonlyMap<string, string>>(new Map());
  const failure = (id: string): string | null => failures().get(id) ?? null;

  const fail = (id: string, cause: string) => setFailures(all => new Map(all).set(id, cause));
  const clear = (id: string) => setFailures(all => all.has(id) ? new Map([...all].filter(([each]) => each !== id)) : all);
  // P3557c: a failure leaves with its record; it never moves to another.
  createEffect(() => {
    const present = new Set(props.state.records().map(record => record.id));
    setFailures(all => [...all.keys()].every(id => present.has(id)) ? all : new Map([...all].filter(([id]) => present.has(id))));
  });
  let disposed = false;
  onCleanup(() => { disposed = true; });

  const open = async (record: Deliverable) => {
    if (pending() !== null || connected.daemonExit() !== null) return;
    setSelected(record.id);
    setPending(record.id);
    clear(record.id);

    try {
      const cause = await openDeliverable(connected.app, record.project, record.id);

      if (!disposed && cause !== null) fail(record.id, cause);
    } catch (error) {
      if (!(error instanceof Error)) throw error;

      if (!disposed) fail(record.id, error.message);
    } finally {
      if (!disposed) setPending(null);
    }
  };

  const openMenu = (record: Deliverable, x: number, y: number) => {
    setSelected(record.id);
    clear(record.id);
    void contextMenu(connected.app, linkEntries(connected.app, { kind: "deliverable", project: record.project, id: record.id },
      connected.rail.nodes, node => connected.rail.exitOf(node) === null, cause => fail(record.id, cause)).map(entry => entry.id !== "copy-link" ? entry : {
        ...entry, label: copy.copy, run: async () => {
          try {
            const resolved = await connected.app.rpc("deliverable.resolve", { project: record.project, id: record.id });

            if (resolved.kind === "unavailable") {
              fail(record.id, copy.deliverableUnavailable[resolved.cause]);

              return;
            }

            const url = copy.linkUrl("deliverable", encodeURIComponent(record.id), encodeURIComponent(record.project));
            await writeText(connected.app, `${copy.linkNote(url)}\n${JSON.stringify({ home: home(record), publication: status(record), record }, null, 2)}`);
            clear(record.id);
          } catch (error) {
            if (!(error instanceof Error)) throw error;
            fail(record.id, copy.copyDraftFailure(error.message));
          }
        },
      }), x, y);
  };

  let section: HTMLElement | undefined;
  registerKeys({ kind: "region", element: () => section }, [{
    chords: ["ContextMenu", "Shift+F10"],
    when: event => event.target instanceof HTMLElement && event.target.matches(".made-open"),
    run: event => {
      if (event.repeat || !(event.target instanceof HTMLElement)) return;
      const id = event.target.closest<HTMLElement>("[data-deliverable]")?.dataset.deliverable;
      const record = records().find(record => record.id === id);

      if (record) {
        const box = event.target.getBoundingClientRect();
        openMenu(record, box.left, box.bottom);
      }
    },
  }]);

  const home = (record: Deliverable) => record.home === null ? copy.overview
    : connected.rail.nodes.find(node => node.id === record.home)?.name ?? copy.unavailableHome;

  const status = (record: Deliverable) => record.target.kind === "external" ? copy.deliverableCurrent
    : record.target.kind === "pad" ? copy.deliverableRevision() : copy.deliverableRetained;

  return <Show when={records().length > 0 || props.state.failure() !== null}>
    <section ref={section} class="made" aria-label={copy.deliverables} aria-busy={pending() !== null || props.state.loading()}>
      <div class="made-heading"><h2 class="overview-heading">{copy.deliverables}</h2>
        <button type="button" data-button="quiet" onClick={() => { void props.state.refresh(); }}>{copy.refresh}</button>
      </div>
      <ReservedLine message={props.state.failure()} />
      <ul class="made-list"><For each={records()}>{record => <li class="made-row" data-deliverable={record.id} data-selected={selected() === record.id}
        onContextMenu={event => openOnRelease(event, (x, y) => openMenu(record, x, y))}>
        <Tip disabled={failure(record.id) !== null} text={copy.openDeliverable(record.title, record.maker_name, home(record), record.published_at)}>{trigger => <button {...trigger()} type="button" data-button="quiet" class="made-open" aria-current={selected() === record.id ? "true" : undefined}
          aria-label={copy.openDeliverable(record.title, record.maker_name, home(record), record.published_at)} aria-disabled={pending() !== null}
          onClick={() => { void open(record); }}>
          <span class="made-content">
            <span class="made-title">{record.title}</span>
            <span class="made-status" data-state={record.target.kind}>{status(record)}</span>
            <span class="made-details">{copy.deliverableAttribution(record.maker_name, home(record))}</span>
            <time class="made-details" datetime={new Date(record.published_at).toISOString()}>{copy.reactionWhen(new Date(record.published_at).toISOString())}</time>
          </span>
          <Icon name="right" />
        </button>}</Tip>
        <ReservedLine overlay message={failure(record.id)}><button type="button" data-button="quiet" onClick={() => clear(record.id)}>{copy.dismiss}</button></ReservedLine>
      </li>}</For></ul>
    </section>
  </Show>;
};
