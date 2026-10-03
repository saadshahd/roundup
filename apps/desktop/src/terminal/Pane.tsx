import { createEffect, createMemo, createSignal, onCleanup, Show, untrack } from "solid-js";
import { ErrorLine } from "../ink/ErrorLine";
import { useConnectedProject } from "../state/connectedProject";
import { createXtermEmulators } from "./emulator";
import type { EmulatorFactory } from "./emulator";
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
  const screens = createScreens(connected, props.createEmulator ?? createXtermEmulators());
  const selected = createMemo(() => rail.nodes.find((node) => node.id === rail.selected()) ?? null);
  const terminalId = createMemo(() => selected()?.terminal_id ?? null);
  /** U38: a failure that fills this region shows in its place, never beside its empty line. */
  const notice = createMemo(() => props.notice ?? screens.failure(terminalId()));
  const [screen, setScreen] = createSignal<HTMLDivElement>();
  let pane: HTMLDivElement | undefined;
  let pendingFrame: number | null = null;

  // Window resize only: opening a Drawer overlays the pane and never changes its size.
  const refit = () => {
    pendingFrame = null;

    const id = terminalId();

    if (id !== null) screens.resize(id, screens.emulatorFor(id).fit());
  };

  const refitNextFrame = () => {
    pendingFrame ??= requestAnimationFrame(refit);
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
    if (!press.metaKey || press.ctrlKey || press.altKey || press.shiftKey) return;

    const target = press.target;

    if (!(target instanceof Element) || !target.closest(".pane-screen")) return;

    if (target.closest('input, textarea, [contenteditable="true"]') && !target.closest(".xterm-helper-textarea")) return;

    const id = terminalId();

    if (id === null) return;

    const key = press.key.toLowerCase();

    if (key !== "c" && key !== "v") return;

    press.preventDefault();
    press.stopPropagation();

    const clipboard = navigator.clipboard;

    if (key === "c") void screens.copy(id, clipboard);
    else void screens.paste(id, clipboard);
  };

  onCleanup(() => pane?.removeEventListener("keydown", onKeyDown, true));

  window.addEventListener("resize", refitNextFrame);
  onCleanup(() => {
    window.removeEventListener("resize", refitNextFrame);

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
          <p class="pane-empty">select an agent or a terminal</p>
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
        <Show when={notice()}>{(message) => <ErrorLine message={message()} />}</Show>
      </div>
    </div>
  );
};
