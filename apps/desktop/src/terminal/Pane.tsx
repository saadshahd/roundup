import { queueFocus } from "../keys/focus";
import { appSettings } from "../settings/preferences";
import { MeasurementDashboard } from "./MeasurementDashboard";
import { TimelineView } from "../timeline/TimelineView";
import { copy } from "../copy";
import { shelfSliding } from "../shelf/sliding";
import { drawerSliding } from "../drawer/sliding";
import { resizeDragging } from "../resize/dragging";
import { For, createEffect, createMemo, createSignal, on, onCleanup, Show, untrack } from "solid-js";
import { OrderDrawer } from "../orders/OrderDrawer";
import { OrderLine } from "../orders/OrderLine";
import { ReservedLine } from "../ink/ReservedLine";
import { NoteQueue } from "./NoteQueue";
import { Icon } from "../ink/Icon";
import { doorStateOf, doorStateText } from "../rail/doorState";
import { useConnectedProject } from "../state/connectedProject";
import { registerKeys } from "../keys/registry";
import { CHORDS } from "../keys/chords";
import { KeyHint } from "../keys/KeyHint";
import { createXtermEmulators } from "./emulator";
import type { EmulatorFactory } from "./emulator";
import { MIN_FONT_SIZE, MAX_FONT_SIZE, fontSizeStorage } from "./fontSize";
import { createScreens } from "./screens";
import type { Screens } from "./screens";
import "./styles.css";

/**
 * The selected row's Terminal. Every error line of the centre lives in the pane's one fixed-height slot, so the
 * screen never moves after it was fitted: `notice` (the Daemon or Rail failure) wins over the pane's own failure.
 * `createEmulator` is the edge tests replace; the App passes none, except the keystroke run (`scenarios/perf.md` K3).
 */
export const Pane = (props: { notice?: string | null; createEmulator?: EmulatorFactory | undefined; screensRef?: (screens: Screens) => void }) => {
  const connected = useConnectedProject();
  const { rail } = connected;
  const storage = fontSizeStorage(connected.project.path);
  let fontSize: number | undefined;
  let zoomed = storage.read() !== null;
  const [storageFailure, setStorageFailure] = createSignal<string | null>(null);
  const createEmulator = props.createEmulator ?? createXtermEmulators();

  const screens = createScreens(connected, (id) => {
    fontSize ??= storage.read() ?? appSettings().fontSize;
    const emulator = createEmulator(id);

    emulator.setFontSize(fontSize);

    return emulator;
  });

  createEffect(on(() => appSettings().fontSize, size => {
    if (!zoomed) { fontSize = size; screens.setFontSize(size); refitNextFrame(); }
  }, { defer: true }));

  createEffect(on(() => appSettings().fontFamily, () => refitNextFrame(), { defer: true }));

  props.screensRef?.(screens);

  const [view, setView] = createSignal<"dashboard" | "timeline" | null>(null);
  const dashboard = () => view() !== null;
  let dashboardAction: HTMLButtonElement | undefined;
  let timelineAction: HTMLButtonElement | undefined;

  const backFromView = () => {
    const action = view() === "timeline" ? timelineAction : dashboardAction;

    setView(null);
    queueFocus(() => action?.focus());
  };

  createEffect(on(rail.selected, () => setView(null), { defer: true }));
  const selected = createMemo(() => rail.nodes.find((node) => node.id === rail.selected()) ?? null);

  /** Why the selected Workstream's Door is not running, so a stopped, failed or still-starting Door each read differently. */
  const doorState = createMemo(() => {
    const workstream = selected();

    return workstream?.kind === "workstream" ? doorStateOf(workstream, rail.exitOf(workstream), rail.doorPending(workstream.id), rail.doorFailure(workstream.id) !== null) : null;
  });

  const closedAgent = createMemo(() => {
    const node = selected();

    return node?.kind === "agent" && rail.exitOf(node) !== null
      && (node.terminal_id !== null || node.status?.kind === "done" || node.status?.kind === "error");
  });

  const terminalId = createMemo(() => selected()?.terminal_id ?? null);
  /** U38: a failure that fills this region shows in its place, never beside its empty line. */
  const notice = createMemo(() => props.notice ?? storageFailure() ?? rail.doorFailure(selected()?.id ?? "") ?? (screens.viewedInBubble(terminalId() ?? "") ? null : screens.failure(terminalId())));
  const [screen, setScreen] = createSignal<HTMLDivElement>();
  let pendingFrame: number | null = null;

  let windowResized = false;

  const refit = () => {
    pendingFrame = null;

    const id = terminalId();

    if (resizeDragging() || drawerSliding() || shelfSliding()) return;

    if (id !== null && connected.daemonExit() === null) screens.resize(id, screens.emulatorFor(id).fit(), !windowResized);

    windowResized = false;
  };

  const refitNextFrame = () => {
    pendingFrame ??= requestAnimationFrame(refit);
  };

  const onWindowResize = () => {
    windowResized = true;
    refitNextFrame();
  };

  // U389, U371: a Resize edge drag changes the pane's width every frame; the Terminal is fitted once when it ends.
  createEffect(on(resizeDragging, (now) => { if (!now) refitNextFrame(); }, { defer: true }));

  // U430: a Drawer's slide narrows or widens the pane over its duration; the Terminal is fitted once when it ends.
  createEffect(on(drawerSliding, (now) => { if (!now) refitNextFrame(); }, { defer: true }));

  createEffect(on(shelfSliding, (now) => { if (!now) refitNextFrame(); }, { defer: true }));

  // X14: the Terminal the selection leaves stops receiving output; only its subscription is released.
  createEffect(on(terminalId, (_id, previous) => { if (previous != null) screens.hide(previous); }, { defer: true }));

  createEffect(() => {
    const host = screen();
    const id = terminalId();

    if (!host) return;

    if (id === null) {
      host.replaceChildren();

      return;
    }

    untrack(() => {
      const show = (focus: boolean) => screens.resize(id, screens.show(id, host, focus));

      if (rail.restored() || !rail.terminalFocus()(() => show(true))) show(false);
    });
  });

  // Solid settles every memo before it runs any effect, so an effect that creates the emulator would still be
  // unrun the first time this reads `isAtBottom` for a newly selected Terminal; ensure it inside the memo instead.
  // The id this control should return to, or null while there is nothing to show: the only guard for both "no
  // Terminal selected" and "already at the bottom", so no two guards can disagree about one decision.
  const latest = createMemo(() => {
    const id = terminalId();

    if (id === null) return null;

    screens.emulatorFor(id);

    return screens.isAtBottom(id) ? null : id;
  });

  /** U106: the Terminal screen's own chords, run by the registry before xterm.js sees them. */
  const inScreen = (press: KeyboardEvent): boolean => {
    const target = press.target;

    return target instanceof Element && screen()?.contains(target) === true
      && !(target.closest('input, textarea, [contenteditable="true"]') && !target.closest(".xterm-helper-textarea"));
  };

  const resize = (key: "grow" | "shrink" | "reset") => (): void => {
    const next = key === "reset" ? appSettings().fontSize : Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, (fontSize ?? appSettings().fontSize) + (key === "grow" ? 1 : -1)));

    if (next === fontSize && key !== "reset") return;

    zoomed = key !== "reset";
    fontSize = next;
    screens.setFontSize(next);
    setStorageFailure(zoomed ? storage.write(next) : storage.clear());
    refitNextFrame();
  };

  const onScreen = (run: (id: string) => void, needsDaemon = false) => ({
    when: (press: KeyboardEvent) => terminalId() !== null && inScreen(press) && (!needsDaemon || connected.daemonExit() === null),
    run: () => run(terminalId()!),
  });

  registerKeys({ kind: "region", element: screen }, [
    { chords: ["Meta+[Shift]+=", "Meta+[Shift]++"], ...onScreen(resize("grow"), true) },
    { chords: ["Meta+-"], ...onScreen(resize("shrink"), true) },
    { chords: ["Meta+0"], ...onScreen(resize("reset"), true) },
  ]);

  /** P3557c: only a Terminal failure is the user's to dismiss; a retention, Rail or Daemon line keeps its own recovery. */
  const dismissFailure = (): (() => void) | null => {
    if (props.notice != null || storageFailure() !== null) return null;

    const id = terminalId();

    return rail.doorFailure(selected()?.id ?? "") === null && id !== null && !screens.viewedInBubble(id) && screens.failure(id) !== null ? () => screens.dismiss(id) : null;
  };

  const focusTerminal = () => {
    const current = terminalId();

    if (current !== null) screens.emulatorFor(current).focus();
  };

  // U162: the window's native File drop, reported by the App seam. The ring shows only while a drop would be taken.
  const [dropOver, setDropOver] = createSignal(false);
  let body: HTMLElement | undefined;

  const takesDrop = (): string | null => {
    const id = terminalId();

    return id !== null && connected.daemonExit() === null && !screens.exited(id) ? id : null;
  };

  const inBody = (x: number, y: number): boolean => {
    const box = body?.getBoundingClientRect();

    if (body === undefined || box === undefined || x < box.left || x >= box.right || y < box.top || y >= box.bottom) return false;

    // An open Drawer sits over the pane, so the rectangle alone would take its drops.
    const hit = document.elementFromPoint?.(x, y);

    return hit === undefined || (hit !== null && body.contains(hit));
  };

  let unlistenDrop: (() => void) | undefined;
  let closed = false;

  void connected.app.onFileDrop((drop) => {
    const id = takesDrop();

    if (drop.phase === "leave" || id === null || !inBody(drop.x, drop.y)) {
      setDropOver(false);

      return;
    }

    setDropOver(drop.phase === "over");

    if (drop.phase === "drop") {
      screens.pastePaths(id, drop.paths);
      screens.emulatorFor(id).focus();
    }
  }).then((stop) => {
    if (closed) stop();
    else unlistenDrop = stop;
  });

  createEffect(on(terminalId, () => setDropOver(false)));
  createEffect(() => { if (takesDrop() === null) setDropOver(false); });
  onCleanup(() => {
    closed = true;
    unlistenDrop?.();
  });

  let watched: ResizeObserver | undefined;

  // A Card appearing, growing or going changes the Terminal's room without a window resize.
  const watch = (body: HTMLElement) => {
    if (typeof ResizeObserver === "undefined") return;

    watched = new ResizeObserver(refitNextFrame);
    watched.observe(body);
    onCleanup(() => watched?.disconnect());
  };

  window.addEventListener("resize", onWindowResize);
  onCleanup(() => {
    window.removeEventListener("resize", onWindowResize);

    if (pendingFrame !== null) cancelAnimationFrame(pendingFrame);

    screens.dispose();
  });

  return (
    <div class="pane">
      <Show when={selected()?.kind === "workstream"}>
        <div class="measurement-action" classList={{ "pane-origin-hidden": dashboard() }}><button ref={dashboardAction} data-button="quiet" type="button" onClick={() => setView("dashboard")}>{copy.measurementTime}</button><button ref={timelineAction} data-button="quiet" type="button" onClick={() => setView("timeline")}>{copy.timeline}</button></div>
      </Show>
      <Show when={view() === "dashboard" && selected()?.kind === "workstream" ? selected()!.id : null} keyed>{(home) => <MeasurementDashboard home={home} name={selected()?.name ?? ""} back={backFromView} />}</Show>
      <Show when={view() === "timeline" && selected()?.kind === "workstream" ? selected()!.id : null} keyed>{(home) => <TimelineView home={home} name={selected()?.name ?? ""} back={backFromView} />}</Show>
      <div class="pane-origin" classList={{ "pane-origin-hidden": dashboard() }}>

      <Show when={selected()?.kind !== "workstream" && !closedAgent() ? selected() : null}>{(node) => <OrderLine node={node()} />}</Show>
      <div
        class="pane-body"
        classList={{ "drop-over": dropOver() }}
        ref={(element) => {
          body = element;
          watch(element);
        }}
      >
        <div class="pane-screen" ref={setScreen} />
        <Show when={selected() === null && notice() === null}>
          <Show when={rail.nodes.length === 0} fallback={<p class="pane-empty">{copy.selectAnAgentOrATerminal}</p>}>
            <div class="pane-empty pane-door">
              <p class="light">{rail.workstreamCreating() ? copy.creatingWorkstream : copy.noWorkstreamYet}</p>
              <button data-button="primary" class="word pane-action" aria-disabled={rail.workstreamCreating() || connected.daemonExit() !== null ? true : undefined} onClick={() => { if (!rail.workstreamCreating() && connected.daemonExit() === null) void rail.createWorkstream(); }}>
                <Icon name="plus" />{copy.newWorkstreamWord}</button>
              <ul class="pane-empty-keys">
                <For each={CHORDS.filter((row) => row.what === copy.newAgent || row.what === copy.newTerminal || row.what === copy.askBarShortcut)}>{(row) => (
                  <li><KeyHint chipAfter keys={row.keys}>{row.what}</KeyHint></li>
                )}</For>
              </ul>
            </div>
          </Show>
        </Show>
        <Show when={selected()?.kind === "agent" && !closedAgent() && terminalId() === null && notice() === null}>
          <p class="pane-empty light">{copy.noOutputKeptFromAnEarlierRun}</p>
        </Show>
        <Show when={closedAgent()}>
          <OrderDrawer id={selected()!.id} inline />
        </Show>
        <Show when={doorState()}>
          {(state) => (
            <div class="pane-empty pane-door">
              <p class="light">{doorStateText(state())}</p>
              <button data-button="primary" class="word pane-action" aria-disabled={rail.doorPending(selected()!.id) || connected.daemonExit() !== null ? true : undefined} onClick={() => { if (!rail.doorPending(selected()!.id) && connected.daemonExit() === null) void rail.startDoor(selected()!.id); }}>
                <Icon name="play" />
                {rail.doorFailure(selected()!.id) ? copy.retry : copy.start}
              </button>
            </div>
          )}
        </Show>
        <Show when={latest()}>
          {(id) => (
            <button data-button="quiet" type="button" class="word pane-latest" onClick={() => screens.returnToBottom(id())}>{copy.latest}</button>
          )}
        </Show>
      </div>
      <Show when={selected()?.kind === "agent" && terminalId() !== null && !closedAgent() ? selected() : null} keyed>{(node) => <NoteQueue node={node} focusTerminal={focusTerminal} />}</Show>
      </div>
      <div class="pane-failure">
        <ReservedLine message={notice() ?? rail.workstreamFailure()}>
          <Show when={dismissFailure()}>{(dismiss) => <button data-button="quiet" type="button" class="word" onClick={() => dismiss()()}>{copy.dismissFailure}</button>}</Show>
        </ReservedLine>
      </div>
    </div>
  );
};
