import { captureFocus } from "../keys/focus";
import { ExitButton } from "../ui/IconButton";
import { createStore, reconcile } from "solid-js/store";
import { createEffect, createMemo, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import type { JSX } from "solid-js";
import type { MeasurementSummary } from "@contracts/project/MeasurementSummary";
import type { MeasurementContributor } from "@contracts/project/MeasurementContributor";
import type { MeasurementTime } from "@contracts/project/MeasurementTime";
import { RpcError } from "../app/seam";
import { copy } from "../copy";
import { useConnectedProject } from "../state/connectedProject";
import { Tip } from "../ui/Tip";
import { duration, percent } from "./measurementFormat";
import "./measurement.styles.css";

const retryAfter = 500;

/** A figure that is only the known part carries a mark that opens its own note. */
const Partial = (props: { note: string }) => <Tip text={props.note}>{(trigger) => <button {...trigger()} data-button="quiet" type="button" class="measurement-mark" aria-label={copy.measurementPartial}>{copy.measurementMark}</button>}</Tip>;

type Category = { key: string; label: string; of: (time: MeasurementTime) => string };

/** The segment order of every row. */
const order: Category[] = [
  { key: "tools", label: copy.measurementTools, of: (time) => time.tools_ns },
  { key: "coordination", label: copy.measurementCoordination, of: (time) => time.coordination_ns },
  { key: "execution", label: copy.measurementExecution, of: (time) => time.execution_ns },
  { key: "other", label: copy.measurementOther, of: (time) => time.other_ns },
  { key: "unmeasured", label: copy.measurementUnmeasured, of: (time) => time.unmeasured_ns },
];

/** `part` of `whole` in thousandths of a percent, exact in nanoseconds. */
const width = (part: bigint, whole: bigint) => (whole > 0n ? Number((part * 100_000n) / whole) / 1000 : 0);

/** The categories with a positive duration, in segment order. */
const positive = (time: MeasurementTime) => {
  const found: { category: Category; ns: bigint }[] = [];

  for (const category of order) {
    const ns = BigInt(category.of(time));

    if (ns > 0n) found.push({ category, ns });
  }

  return found;
};

const partial = (row: MeasurementContributor) => [...row.time.coverage, ...row.coverage];

const Row = (props: { row: MeasurementContributor; longest: bigint; open: (row: MeasurementContributor) => void }) => {
  const segments = () => positive(props.row.time);
  const summed = () => BigInt(props.row.time.summed_ns);
  const unknown = () => partial(props.row).length > 0;

  return <li class="measurement-row">
    <Tip text={props.row.maker}>{(trigger) => <button {...trigger()} data-button="quiet" type="button" class="measurement-agent" onClick={() => props.open(props.row)}>{props.row.maker}</button>}</Tip>
    <div class="measurement-bar"><Show when={segments().length > 0}><div class="measurement-stack" style={{ width: `${width(segments().reduce((sum, segment) => sum + segment.ns, 0n), props.longest)}%` }}>
      <For each={segments()}>{(segment) => {
        const name = copy.measurementSegment(props.row.maker, segment.category.label, duration(segment.ns.toString()), percent(Number(segment.ns), Number(summed())));

        return <Tip text={name}>{(trigger) => <span {...trigger()} role="img" aria-label={name} tabIndex={0} class="measurement-segment" data-category={segment.category.key} style={{ "flex-grow": segment.ns.toString() }} />}</Tip>;
      }}</For>
    </div></Show></div>
    <span class="measurement-duration">{segments().length === 0 && unknown() ? copy.measurementUnknown : duration(props.row.time.summed_ns)}<Show when={unknown()}><Partial note={[copy.measurementPartialTime, ...partial(props.row)].join(" ")} /></Show></span>
  </li>;
};

/** The current Workstream owns one reader, revision and subscription. Unmount drops late replies. */
export const MeasurementDashboard = (props: { home: string; name: string; back: () => void }) => {
  const connected = useConnectedProject();
  const [data, setData] = createStore<{ snapshot: MeasurementSummary | null }>({ snapshot: null });
  const [pending, setPending] = createSignal(false);
  const [failures, setFailures] = createSignal(0);
  const [failure, setFailure] = createSignal<string | null>(null);
  const [removed, setRemoved] = createSignal<string | null>(null);
  const [visible, setVisible] = createSignal(document.visibilityState !== "hidden");

  const snapshot = () => data.snapshot;
  // A work-running Workstream follows new measurements; a stopped one shows its last (U3016i).
  const live = () => snapshot()?.time.active === true && connected.daemonExit() === null;
  // One failed read followed by a good one shows nothing (U3016j): only a repeat is a problem.
  const problem = () => (failures() >= 2 ? failure() : snapshot()?.cause ?? null);

  let generation = 0;
  let closed = false;
  let region: HTMLElement | undefined;
  let loadingMore = false;
  let queued = false;
  let retryMore = false;
  let retryTimer: ReturnType<typeof setTimeout> | undefined;

  const fail = (cause: string) => {
    setFailure(cause);
    setFailures((count) => count + 1);

    if (failures() === 1 && !closed) {
      clearTimeout(retryTimer);
      retryTimer = setTimeout(() => void read(retryMore), retryAfter);
    }
  };

  const read = async (more = false) => {
    if (loadingMore) {
      queued = true;

      return;
    }

    const before = snapshot();

    if (more && (before === null || before.next_offset === null)) return;
    const ticket = ++generation;
    const count = before?.rows.length ?? 0;
    loadingMore = true;
    setPending(true);

    try {
      const next = await connected.app.rpc("project.measurementSummary", { home: props.home, offset: more ? before!.next_offset! : 0, revision: more ? before!.revision : null });

      if (closed || ticket !== generation) return;
      const current = snapshot();

      if (next.home !== props.home || (current !== null && BigInt(next.revision) < BigInt(current.revision))) return;

      if (more && current?.revision !== next.revision) {
        queued = true;

        return;
      }

      // A revision update invalidates all loaded pages together. Same-revision timer reads
      // preserve those pages and their DOM/focus, rather than replacing them with twenty rows.
      const rows = more ? [...before!.rows, ...next.rows] : current?.revision === next.revision && count >= next.rows.length ? current.rows : next.rows;
      setData("snapshot", reconcile({ ...next, rows, next_offset: !more && rows.length > next.rows.length ? current!.next_offset : next.next_offset }, { key: "agent" }));
      retryMore = false;
      clearTimeout(retryTimer);
      setFailures(0);
      setFailure(null);
    } catch (error) {
      if (!(error instanceof Error)) throw error;

      if (!closed && ticket === generation) {
        retryMore = more;

        if (more && error instanceof RpcError && error.code === -32003) {
          retryMore = false;
          queued = true;
        }

        fail(error.message);
      }
    } finally {
      loadingMore = false;

      if (!closed) {
        setPending(false);

        if (queued) { queued = false; void read(); }
      }
    }
  };

  const changedVisibility = () => setVisible(document.visibilityState !== "hidden");
  document.addEventListener("visibilitychange", changedVisibility);

  const stop = connected.events.subscribe((event) => {
    if (event.name === "project.measurementsChanged") void read();
  });

  onMount(() => void read());
  createEffect(() => {
    if (connected.daemonExit() !== null) { ++generation; setFailure(copy.measurementDaemonEnded); setFailures(2); }
  });
  createEffect(() => {
    if (!visible() || !live()) return;
    const timer = setInterval(() => void read(), 1000);
    onCleanup(() => clearInterval(timer));
  });
  const initialFocus = captureFocus();
  onMount(() => initialFocus(() => region?.focus()));
  onCleanup(() => { closed = true; ++generation; clearTimeout(retryTimer); stop(); document.removeEventListener("visibilitychange", changedVisibility); });

  const rows = () => snapshot()?.rows ?? [];
  const longest = createMemo(() => rows().reduce((most, row) => (BigInt(row.time.summed_ns) > most ? BigInt(row.time.summed_ns) : most), 0n));
  // One chart, or one line saying nothing was measured: no row has a positive category.
  const measured = createMemo(() => rows().some((row) => positive(row.time).length > 0));
  const key = createMemo(() => (snapshot() ? positive(snapshot()!.time) : []));

  const open = (row: MeasurementContributor) => {
    if (row.removed || !connected.rail.nodes.some((node) => node.id === row.agent)) setRemoved(row.maker);
    else { connected.rail.select(row.agent); props.back(); }
  };

  const header = (): JSX.Element => {
    const value = snapshot();

    if (problem()) return <Tip text={copy.measurementProblem(problem()!)}>{(trigger) => <span {...trigger()} class="measurement-state" data-state="stale">{copy.measurementProblem(problem()!)}</span>}</Tip>;

    if (value === null) return <span class="measurement-state">{copy.measurementLoading}</span>;

    return <span class="measurement-state" />;
  };

  const retry = () => <Show when={problem()}><button data-button="quiet" type="button" aria-disabled={pending()} onClick={() => { if (!pending()) void read(retryMore); }}>{copy.retry}</button></Show>;

  const note = () => [copy.measurementPartialTime, ...(snapshot()?.time.coverage ?? [])].join(" ");

  return <section class="measurement-dashboard" ref={region} role="region" aria-label={copy.measurementTime} aria-busy={pending()} data-home={props.home} data-revision={snapshot()?.revision} tabIndex={-1} onKeyDown={(event) => { if (event.key === "Escape" && !event.defaultPrevented) { event.stopPropagation(); props.back(); } }}>
    <header class="measurement-header"><h2>{props.name}</h2>{header()}<div class="measurement-actions">{retry()}<ExitButton onClick={props.back} /></div></header>
    <p class="measurement-line">{copy.measurementElapsed(duration(snapshot()?.time.elapsed_ns ?? null))}<Show when={snapshot() && snapshot()!.time.elapsed_ns === null}><Partial note={note()} /></Show>
      <Show when={snapshot()}>{(value) => <>{copy.measurementSeparator}{copy.measurementAgentTime(duration(value().time.summed_ns))}<Show when={value().time.coverage.length > 0}><Partial note={note()} /></Show></>}</Show></p>
    <section class="measurement-chart" aria-label={copy.measurementTimeSpent}>
      <Show when={snapshot() !== null}>
        <Show when={measured()} fallback={<p class="measurement-note">{copy.measurementEmpty}</p>}>
          <ul class="measurement-key"><For each={key()}>{(entry) => <li data-category={entry.category.key}><span class="measurement-swatch" />{copy.measurementKeyEntry(entry.category.label, duration(entry.ns.toString()), percent(Number(entry.ns), Number(BigInt(snapshot()!.time.summed_ns))))}</li>}</For></ul>
          <ol class="measurement-rows"><For each={rows()}>{(row) => <Row row={row} longest={longest()} open={open} />}</For></ol>
        </Show>
      </Show>
      <Show when={removed()}>{(name) => <p class="measurement-note">{copy.measurementRemoved(name())}</p>}</Show>
      <Show when={snapshot()?.next_offset != null}><button data-button="quiet" type="button" aria-disabled={pending()} onClick={() => { if (!pending()) void read(true); }}>{copy.measurementLoadMore}</button></Show>
    </section>
  </section>;
};
