import { createEffect, createMemo, createSignal, onCleanup, Show, untrack } from "solid-js";
import { ErrorLine } from "../ink/ErrorLine";
import { Icon } from "../ink/Icon";
import { doorStateOf, doorStateText } from "../rail/doorState";
import { useConnectedProject } from "../state/connectedProject";
import { createXtermEmulators } from "./emulator";
import type { EmulatorFactory } from "./emulator";
import { DEFAULT_FONT_SIZE, MIN_FONT_SIZE, MAX_FONT_SIZE, fontSizeStorage } from "./fontSize";
import { createScreens } from "./screens";
import "./styles.css";

/**
 * The selected row's Terminal. Every error line of the centre lives in the pane's one fixed-height slot, so the
 * screen never moves after it was fitted: `notice` (the Daemon or Rail failure) wins over the pane's own failure.
 * `createEmulator` is the edge tests replace; the App passes none, except the keystroke run (`scenarios/perf.md` K3).
 */
export const Pane = (props: { notice?: string | null; createEmulator?: EmulatorFactory | undefined }) => {
  const connected = useConnectedProject();
  const { rail } = connected;
  const storage = fontSizeStorage(connected.project.path);
  let fontSize: number | undefined;
  const [storageFailure, setStorageFailure] = createSignal<string | null>(null);
  const createEmulator = props.createEmulator ?? createXtermEmulators();

  const screens = createScreens(connected, (id) => {
    fontSize ??= storage.read() ?? DEFAULT_FONT_SIZE;
    const emulator = createEmulator(id);

    emulator.setFontSize(fontSize);

    return emulator;
  });

  const selected = createMemo(() => rail.nodes.find((node) => node.id === rail.selected()) ?? null);

  /** Why the selected Room's Door is not running, so a stopped, failed or still-starting Door each read differently. */
  const doorState = createMemo(() => {
    const room = selected();

    return room?.kind === "room" ? doorStateOf(room, rail.exitOf(room), rail.doorPending(room.id), rail.doorFailure(room.id) !== null) : null;
  });

  const terminalId = createMemo(() => selected()?.terminal_id ?? null);
  /** U38: a failure that fills this region shows in its place, never beside its empty line. */
  const notice = createMemo(() => props.notice ?? storageFailure() ?? rail.doorFailure(selected()?.id ?? "") ?? screens.failure(terminalId()));
  const [screen, setScreen] = createSignal<HTMLDivElement>();
  let pane: HTMLDivElement | undefined;
  let pendingFrame: number | null = null;

  let windowResized = false;

  const refit = () => {
    pendingFrame = null;

    const id = terminalId();

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

  createEffect(() => {
    const host = screen();
    const id = terminalId();

    if (!host) return;

    if (id === null) {
      host.replaceChildren();

      return;
    }

    untrack(() => screens.resize(id, screens.show(id, host, !rail.restored())));
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

  const onKeyDown = (press: KeyboardEvent): void => {
    if (!press.metaKey || press.ctrlKey || press.altKey) return;

    const target = press.target;

    if (!(target instanceof Element) || !target.closest(".pane-screen")) return;

    if (target.closest('input, textarea, [contenteditable="true"]') && !target.closest(".xterm-helper-textarea")) return;

    const id = terminalId();

    if (id === null) return;

    const key = press.key.toLowerCase();

    const grows = key === "=" || key === "+";

    if (press.shiftKey && !grows) return;

    if (grows || key === "-" || key === "0") {
      if (connected.daemonExit() !== null) return;

      press.preventDefault();
      press.stopPropagation();

      const next = key === "0" ? DEFAULT_FONT_SIZE : Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, (fontSize ?? DEFAULT_FONT_SIZE) + (grows ? 1 : -1)));

      if (next === fontSize) return;

      fontSize = next;
      screens.setFontSize(next);
      setStorageFailure(storage.write(next));
      refitNextFrame();

      return;
    }

    if (key !== "c" && key !== "v") return;

    press.preventDefault();
    press.stopPropagation();

    const clipboard = navigator.clipboard;

    if (key === "c") void screens.copy(id, clipboard);
    else void screens.paste(id, clipboard);
  };

  onCleanup(() => pane?.removeEventListener("keydown", onKeyDown, true));

  window.addEventListener("resize", onWindowResize);
  onCleanup(() => {
    window.removeEventListener("resize", onWindowResize);

    if (pendingFrame !== null) cancelAnimationFrame(pendingFrame);

    screens.dispose();
  });

  return (
    <div class="pane" ref={(element) => {
      pane = element;
      element.addEventListener("keydown", onKeyDown, true);
    }}>
      <div class="pane-body">
        <div class="pane-screen" ref={setScreen} />
        <Show when={selected() === null && notice() === null}>
          <Show when={rail.nodes.length === 0} fallback={<p class="pane-empty">select an agent or a terminal</p>}>
            <div class="pane-empty pane-door">
              <p class="light">{rail.roomCreating() ? "creating Room…" : "no Room yet"}</p>
              <button class="word pane-action" disabled={rail.roomCreating() || connected.daemonExit() !== null} onClick={() => void rail.createRoom()}>
                <Icon name="plus" /> start a Room
              </button>
            </div>
          </Show>
        </Show>
        <Show when={doorState()}>
          {(state) => (
            <div class="pane-empty pane-door">
              <p class="light">{doorStateText(state())}</p>
              <button class="word pane-action" disabled={rail.doorPending(selected()!.id) || connected.daemonExit() !== null} onClick={() => void rail.startDoor(selected()!.id)}>
                <Icon name="right" />
                {rail.doorFailure(selected()!.id) ? "retry Door" : "start Door"}
              </button>
            </div>
          )}
        </Show>
        <Show when={latest()}>
          {(id) => (
            <button type="button" class="word pane-latest" onClick={() => screens.returnToBottom(id())}>
              ↓ latest
            </button>
          )}
        </Show>
      </div>
      <div class="pane-failure">
        <Show when={notice() ?? rail.roomFailure()}>{(message) => <ErrorLine message={message()} />}</Show>
      </div>
    </div>
  );
};
