import { captureFocus } from "../keys/focus";
import { Show } from "solid-js";
import { ReservedLine } from "../ink/ReservedLine";
import { copy } from "../copy";
import { createSignal } from "solid-js";
import type { Accessor, JSX } from "solid-js";
import type { TerminalInfo } from "@contracts/terminal/TerminalInfo";
import type { ConnectedProject } from "../state/connectedProject";

/** Terminal closure preserves the surviving tab focus and leaves failures on the originating surface. */
export const createTerminalClose = (connected: ConnectedProject, shown: Accessor<readonly TerminalInfo[]>, nameFor: (terminal: TerminalInfo) => string, setFailure: (message: string | null, terminal?: TerminalInfo) => void, clearItemFailure: (terminal: TerminalInfo) => void) => {
  const { app, rail } = connected;
  const [closing, setClosing] = createSignal<ReadonlySet<string>>(new Set());
  const [retry, setRetry] = createSignal<{ id: string; message: string } | null>(null);

  const close = async (id: string): Promise<void> => {
    if (closing().has(id)) return;

    const terminal = shown().find(terminal => terminal.id === id);
    const focus = captureFocus();
    const active = document.activeElement;
    const controlHadFocus = active instanceof Element && active.closest("[data-bubble]")?.getAttribute("data-bubble") === id;
    setClosing((ids) => new Set([...ids, id]));

    try {
      if (!terminal) return;
      const accepted = await app.ask({ title: copy.closeQuestion(nameFor(terminal)), message: terminal.running ? copy.closeTerminalWarning : copy.closeTerminalOutputWarning, okLabel: copy.close, cancelLabel: copy.keep });

      if (!accepted) {
        focus(() => { if (active instanceof HTMLElement && active.isConnected) active.focus({ preventScroll: true }); });

        return;
      }

      await app.rpc("terminal.close", { id });
      await rail.refresh({ terminals: true });
      setRetry((prior) => prior?.id === id ? null : prior);
      clearItemFailure(terminal);

      if (controlHadFocus) queueMicrotask(() => focus(() => {
        const remaining = shown();
        const nextId = remaining.find((terminal) => Number(terminal.id) > Number(id))?.id ?? remaining.at(-1)?.id;
        const control = nextId === undefined ? document.querySelector<HTMLElement>('.terminal-new') : document.querySelector<HTMLElement>(`.terminal-tab[data-bubble="${nextId}"]`);
        control?.focus();
      }));
    } catch (thrown) {
      if (!(thrown instanceof Error)) throw thrown;

      setRetry({ id, message: thrown.message });
      setFailure(thrown.message, terminal);
    } finally {
      setClosing((ids) => new Set([...ids].filter((prior) => prior !== id)));
    }
  };

  const Failure = (props: { message: string | null; children?: JSX.Element }) => <ReservedLine overlay class={props.message ? "bubble-failure" : undefined} message={props.message}>
    <Show when={retry()?.message === props.message ? retry() : null}>{failed => <button data-button="quiet" type="button" onClick={() => {
      // Retry starts from the original close control, so cancel and success keep their normal focus destinations.
      document.querySelector<HTMLElement>(`.terminal-tab-pair[data-bubble="${failed().id}"] button:last-child`)?.focus();
      void close(failed().id);
    }}>{copy.retry}</button>}</Show>
    {props.children}
  </ReservedLine>;

  return { close, closing, Failure };
};
