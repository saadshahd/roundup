import { captureFocus } from "../keys/focus";
import { ReservedLine } from "../ink/ReservedLine";
import { createEffect, createSignal, For, on, onCleanup, Show } from "solid-js";
import type { Ref } from "@contracts/todo/Ref";
import { copy } from "../copy";
import { activationSource } from "../keys/registry";
import { PadDrawer } from "../pads/PadDrawer";
import { usePadList } from "../rail/pads/padList";
import { useConnectedProject } from "../state/connectedProject";
import { createCutTitle } from "../ui/cutTitle";
import { doesClearTodoFailure, failureOf } from "./failureOf";
import { refTarget } from "./refTarget";

const PadAt = (props: { name: string; heading: string | null }) => {
  const list = usePadList();

  return <PadDrawer name={props.name} heading={props.heading ?? undefined} pads={list.pads} onLateFailure={() => {}} />;
};

/** U3760: the Todo's Pad links in the order added. An `ok` one opens its Pad; a `missing` one is text. An Agent's change shows without reopening. */
export const TodoRefs = (props: { id: number; saved?: () => number; onSaved?: () => void }) => {
  const { app, drawer, events } = useConnectedProject();
  const [refs, setRefs] = createSignal<readonly Ref[]>([]);
  const [failure, setFailure] = createSignal<string | null>(null);
  const [actionFailure, setActionFailure] = createSignal<string | null>(null);
  createEffect(on(props.saved ?? (() => 0), () => setActionFailure(null), { defer: true }));
  let latest = 0;
  let alive = true;

  // Only the newest fetch may write, so overlapping answers never put an older list back.
  const refresh = async () => {
    const mine = ++latest;
    const id = props.id;

    const message = await failureOf(async () => {
      const answered = await app.rpc("todo.refs", { id });

      if (alive && mine === latest && id === props.id) setRefs(answered);
    });

    if (alive && mine === latest && id === props.id) setFailure(message);
  };

  createEffect(on(() => props.id, () => { setRefs([]); setFailure(null); setActionFailure(null); void refresh(); }));

  // A Pad that changes or goes away moves a ref between `ok` and `missing`.
  onCleanup(events.subscribe((event) => {
    if (doesClearTodoFailure(event, props.id)) setActionFailure(null);

    if (event.name === "pad.changed" || event.name === "pad.deleted") void refresh();
    else if ((event.name === "todo.refAdded" || event.name === "todo.refRemoved") && event.data.id === props.id) void refresh();
  }));
  onCleanup(() => { alive = false; latest++; });

  const remove = async (url: string, button: HTMLElement) => {
    const focus = captureFocus();
    const section = button.closest<HTMLElement>(".todo-refs");
    // With no row left, the section unmounts: focus goes to the first control after it.
    const parent = section?.parentElement;
    const index = section ? [...(parent?.children ?? [])].indexOf(section) : -1;
    const rows = [...(section?.querySelectorAll<HTMLElement>(".todo-ref") ?? [])];
    // SAFETY: the button sits inside its .todo-ref row.
    const at = rows.indexOf(button.closest(".todo-ref") as HTMLElement);

    const id = props.id;
    const message = await failureOf(async () => { await app.rpc("todo.removeRef", { id, url }); });

    if (!alive || id !== props.id) return;
    setActionFailure(message);

    if (message !== null) return;
    props.onSaved?.();
    await refresh();
    // The row is gone; focus its next (else previous) row so the user keeps their place.
    const left = [...(section?.isConnected ? section.querySelectorAll<HTMLElement>(".todo-ref") : [])];
    const row = left[Math.min(at, left.length - 1)];

    focus(() => {
      if (document.activeElement === document.body || !document.activeElement?.isConnected) {
        if (row) (row.querySelector<HTMLElement>(".todo-ref-link") ?? row.querySelector<HTMLElement>("button"))?.focus();
        else {
          // The section unmounted: the first control among what now sits where it was.
          const following = [...(parent?.children ?? [])].slice(index).filter((next) => next instanceof HTMLElement);
          const control = following.flatMap((next) => [next, ...next.querySelectorAll<HTMLElement>("button,a[href]")]).find((found) => found.matches("button,a[href]"));
          control?.focus();
        }
      }
    });
  };

  return <>
    <ReservedLine message={actionFailure() ?? failure()}><button data-button="quiet" class="word" type="button" onClick={() => void refresh()}>{copy.padRefRetry}</button><button data-dismiss-failure onMouseDown={event => event.preventDefault()} data-button="quiet" class="word" type="button" onClick={() => { setActionFailure(null); setFailure(null); }}>{copy.dismissFailure}</button></ReservedLine>
    <Show when={refs().length > 0}>
    <section class="todo-refs" aria-label={copy.padRefsHeading(refs().length)}>
      <h3>{copy.padRefsHeading(refs().length)}</h3>

        <For each={refs()}>{(ref) => {
          const target = refTarget(ref.url);
          const linkCut = createCutTitle(() => target.name);
          const label = <>{target.name}{target.heading !== null && <span class="todo-ref-heading light">{copy.padRefHeading(target.heading)}</span>}</>;

          return <div class="todo-ref" data-state={ref.state}>
            <Show when={ref.state === "ok"} fallback={<span class="todo-ref-text">{label}<span class="todo-ref-state">{copy.padRefMissingState}</span></span>}>
              <button {...linkCut.attrs()} data-button="quiet" type="button" role="link" class="word todo-ref-link" onClick={(event) => drawer.open(() => <PadAt name={target.name} heading={target.heading} />, activationSource(event))}>{label}</button>
            </Show>
            <button data-button="quiet" type="button" class="word light" aria-label={copy.removePadRef(target.name)} onClick={(event) => void remove(ref.url, event.currentTarget)}>{copy.remove}</button>
          </div>;
        }}</For>
    </section>
  </Show></>;
};
