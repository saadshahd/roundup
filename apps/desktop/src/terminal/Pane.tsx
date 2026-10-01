import { createEffect, createMemo, createSignal, onCleanup, Show, untrack } from "solid-js";
import { ErrorLine } from "../ink/ErrorLine";
import { useConnectedProject } from "../state/connectedProject";
import { createXtermEmulators } from "./emulator";
import type { EmulatorFactory } from "./emulator";
import { paneHeader } from "./header";
import { createScreens } from "./screens";
import "./pane.css";

/** The selected row's Terminal. `createEmulator` is the edge tests replace; the App passes none. */
export const Pane = (props: { createEmulator?: EmulatorFactory }) => {
  const connected = useConnectedProject();
  const { rail } = connected;
  const screens = createScreens(connected, props.createEmulator ?? createXtermEmulators());
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

  window.addEventListener("resize", refitNextFrame);
  onCleanup(() => {
    window.removeEventListener("resize", refitNextFrame);

    if (pendingFrame !== null) cancelAnimationFrame(pendingFrame);

    screens.dispose();
  });

  return (
    <div class="pane">
      <Show when={selected()}>
        {(node) => (
          <Show when={paneHeader(node(), rail.exitOf(node()), connected.now())}>
            {(header) => <p class="pane-header">{header()}</p>}
          </Show>
        )}
      </Show>
      <Show when={screens.failure()}>{(message) => <ErrorLine message={message()} />}</Show>
      <div class="pane-screen" ref={setScreen} />
    </div>
  );
};
