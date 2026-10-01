import { createEffect, createMemo, createSignal, onCleanup, Show, untrack } from "solid-js";
import { ErrorLine } from "../ink/ErrorLine";
import { useWorkspace } from "../state/workspace";
import { createXtermEmulators } from "./emulator";
import type { EmulatorFactory } from "./emulator";
import { paneHeader } from "./header";
import { createScreens } from "./screens";
import "./pane.css";

const CLOCK_TICK_MS = 30_000;

/** The selected row's Terminal. `createEmulator` and `now` are the edges tests replace; the App passes neither. */
export const Pane = (props: { createEmulator?: EmulatorFactory; now?: () => number }) => {
  const workspace = useWorkspace();
  const { rail } = workspace;
  const screens = createScreens(workspace, props.createEmulator ?? createXtermEmulators());
  const clock = props.now ?? Date.now;
  const [now, setNow] = createSignal(clock());
  const selected = createMemo(() => rail.nodes.find((node) => node.id === rail.selected()) ?? null);
  const terminalId = createMemo(() => selected()?.terminal_id ?? null);
  const [screen, setScreen] = createSignal<HTMLDivElement>();
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

    untrack(() => screens.resize(id, screens.emulatorFor(id).show(host)));
  });

  const clockTick = setInterval(() => setNow(clock()), CLOCK_TICK_MS);

  window.addEventListener("resize", refitNextFrame);
  onCleanup(() => {
    window.removeEventListener("resize", refitNextFrame);

    if (pendingFrame !== null) cancelAnimationFrame(pendingFrame);

    clearInterval(clockTick);
    screens.dispose();
  });

  return (
    <div class="pane">
      <Show when={selected()}>
        {(node) => (
          <Show when={paneHeader(node(), rail.exitOf(node()), now())}>
            {(header) => <p class="pane-header">{header()}</p>}
          </Show>
        )}
      </Show>
      <Show when={screens.failure()}>{(message) => <ErrorLine message={message()} />}</Show>
      <div class="pane-screen" ref={setScreen} />
    </div>
  );
};
