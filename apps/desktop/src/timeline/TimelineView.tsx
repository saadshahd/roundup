import { queueFocus } from "../keys/focus";
import { Tip } from "../ui/Tip";
import { createCutTitle } from "../ui/cutTitle";
import { ExitButton } from "../ui/IconButton";
import { createEffect, createMemo, createSignal, For, on, onCleanup, Show } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import type { Kind } from "@contracts/Kind";
import { copy } from "../copy";
import { glyphOf } from "../ink/glyph";
import { LABELS } from "../ink/KindGlyph";
import { useConnectedProject } from "../state/connectedProject";
import { blocksOf, WINDOW_MS } from "./model";
import type { Block } from "./model";
import "./styles.css";

/** Reduced motion keeps the window still and leaves this much room past its last redraw for the open block to grow into. */
const HEADROOM_MS = 5 * 60_000;

const MINUTE_MS = 60_000;

const SpanLabel = (props: { kind: Kind }) => {
  const fit = createCutTitle(() => LABELS[props.kind]);

  return <span class="timeline-block-label" ref={fit.ref} aria-hidden="true" style={{ visibility: fit.title() ? "hidden" : "visible" }}>{LABELS[props.kind]}</span>;
};

/**
 * U3768a to U3768f: one lane per Agent of the Workstream `home` over the last 30 minutes.
 * It reads the Project's timeline log, which holds the `agent.status` Events heard since connecting and the Rail's current Statuses; an Agent's time before both is unknown.
 * At most one display clock lives here, and it ends with the view.
 */
export const TimelineView = (props: { home: string; name: string; back: () => void; clock?: () => number }) => {
  const connected = useConnectedProject();
  const { rail } = connected;
  const clock = props.clock ?? Date.now;
  const [now, setNow] = createSignal(clock());
  const [anchor, setAnchor] = createSignal(clock());
  const [visible, setVisible] = createSignal(document.visibilityState !== "hidden");

  const redraw = () => {
    const at = clock();

    setNow(at);
    setAnchor(at);
  };

  const { samples, known } = connected.timeline;

  // A change the log heard redraws; the first run is the view opening.
  createEffect(on(connected.timeline.version, () => redraw(), { defer: true }));

  const onVisibility = () => {
    const shown = document.visibilityState !== "hidden";

    setVisible(shown);

    if (shown) redraw();
  };

  document.addEventListener("visibilitychange", onVisibility);

  const reduced = () => connected.reducedMotion();
  const axisEnd = () => reduced() ? anchor() + HEADROOM_MS : now();
  const axisStart = () => axisEnd() - WINDOW_MS;
  const shownEnd = () => reduced() ? Math.min(Math.floor(now() / MINUTE_MS) * MINUTE_MS, axisEnd()) : now();

  type Lane = { id: string; name: string; removed: boolean; parent: string | null; blocks: (Block & { id: string })[] };

  const derived = createMemo((): Lane[] => {
    const children = (parent: string | null) => Object.entries(known).filter(([, each]) => each.parent === parent).toSorted((a, b) => (a[1].removed ? 1 : 0) - (b[1].removed ? 1 : 0) || a[1].order - b[1].order).map(([id]) => id);
    const walk = (id: string): string[] => [id, ...children(id).flatMap(walk)];
    const ids = known[props.home] ? walk(props.home) : [];

    return ids.flatMap((id) => {
      const blocks = blocksOf(samples[id] ?? [], 0).map((block) => ({ ...block, id: `${block.attempt}:${block.start}` }));

      return (samples[id]?.length ?? 0) === 0 ? [] : [{ id, name: known[id]!.name, removed: known[id]!.removed, parent: id === props.home ? null : known[id]!.parent, blocks }];
    });
  });

  // Lanes and blocks keep their elements across redraws, so focus on a block survives a tick.
  const [shown, setShown] = createStore<{ lanes: Lane[] }>({ lanes: [] });

  createEffect(() => setShown("lanes", reconcile(derived(), { key: "id" })));
  const lanes = () => shown.lanes;
  const endOf = (block: Block) => block.open ? shownEnd() : block.end;
  const running = () => lanes().some((lane) => lane.blocks.some((block) => block.open && block.kind === "working"));

  // The display clock: one timer, only while a shown Agent works and the window is visible (U3768e).
  createEffect(() => {
    if (!visible() || !running() || connected.daemonExit() !== null) return;
    const timer = setInterval(() => setNow(clock()), 1000);

    onCleanup(() => clearInterval(timer));
  });

  createEffect(on(reduced, () => redraw(), { defer: true }));

  const percent = (at: number) => `${(Math.max(0, Math.min(1, (at - axisStart()) / WINDOW_MS)) * 100).toFixed(3)}%`;

  const lines = () => lanes().flatMap((child, at) => {
    if (child.blocks.length === 0) return [];
    const parent = lanes().findIndex((lane) => lane.id === child.parent);

    return parent < 0 ? [] : [{ from: parent, to: at, at: child.blocks[0]!.start, label: copy.timelineStarted(lanes()[parent]!.name, child.name) }];
  });

  const waiting = (block: { open: boolean; kind: Kind }) => block.open && block.kind === "needs-you";

  onCleanup(() => {
    document.removeEventListener("visibilitychange", onVisibility);
  });

  /** An open needs-you block selects the Agent's Terminal, where its native question waits. */
  const toAgent = (id: string) => {
    rail.select(id);
    props.back();
  };

  return <section class="timeline-view" classList={{ "timeline-single": lanes().length === 1 }} ref={(element) => queueFocus(() => element.focus())} role="region" aria-label={copy.timeline} data-window-start={Math.round(axisStart())} data-window-end={Math.round(axisEnd())} data-running={running() ? "" : undefined} tabIndex={-1}
    onKeyDown={(event) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      event.stopPropagation();

      props.back();
    }}>
    <header class="timeline-header"><h2>{props.name}</h2><ExitButton onClick={props.back} /></header>
    <Show when={lanes().length > 0} fallback={<p class="empty-state timeline-empty">{copy.timelineEmpty}</p>}>
      <div class="timeline-axis"><span /><div class="timeline-axis-ticks"><For each={[30, 20, 10, 0]}>{(minutes) => <span style={{ left: `${(30 - minutes) / 30 * 100}%` }}>{minutes === 0 ? copy.timelineNow : copy.timelineAgo(minutes)}</span>}</For></div></div>
      <div class="timeline-lanes">
        <div class="timeline-lines">
          <For each={lines()}>{(line) => <span class="timeline-line" role="img" aria-label={line.label} style={{ left: percent(line.at), top: `calc(var(--lane-height) * ${line.from + 0.5})`, height: `calc(var(--lane-height) * ${line.to - line.from})` }} />}</For>
        </div>
        <ol aria-label={copy.timelineLanes}>
          <For each={lanes()}>{(lane) => <li class="timeline-lane" data-agent={lane.id}>
            <span class="timeline-name"><Show when={lane.id !== props.home && lane.name !== props.name}>{lane.removed ? copy.timelineRemoved(lane.name) : lane.name}</Show></span>
            <div class="timeline-track" role="group" aria-label={copy.timelineTrack(lane.removed ? copy.timelineRemoved(lane.name) : lane.name)}>
              <Show when={(samples[lane.id]?.[0]?.at ?? Infinity) > axisStart()}><span class="timeline-unknown" role="img" aria-label={copy.timelineUnknown} /></Show>
              <For each={lane.blocks}>{(block) => {
                const style = () => ({ left: percent(block.start), width: `calc(${percent(endOf(block))} - ${percent(block.start)})` });
                const label = () => LABELS[block.kind];

                return <Show when={waiting(block)} fallback={<span class="timeline-block" role="img" aria-label={label()} data-kind={block.kind} data-tone={glyphOf(block.kind).tone} data-attempt={block.attempt} data-open={block.open ? "" : undefined} data-start={block.start} data-end={Math.round(endOf(block))} hidden={endOf(block) <= axisStart()} style={style()}><SpanLabel kind={block.kind} /></span>}>
                  <Tip text={copy.timelineOpenTerminal(lane.name)}>{(trigger) => <button {...trigger()} class="timeline-block" type="button" aria-label={copy.timelineOpenTerminal(lane.name)} data-kind={block.kind} data-tone={glyphOf(block.kind).tone} data-open="" data-start={block.start} data-end={Math.round(endOf(block))} hidden={endOf(block) <= axisStart()} style={style()} onClick={() => toAgent(lane.id)}><SpanLabel kind={block.kind} /></button>}</Tip>
                </Show>;
              }}</For>
            </div>
          </li>}</For>
        </ol>
      </div>
    </Show>
  </section>;
};
