import { createEffect, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { ErrorLine } from "../ink/ErrorLine";
import { useConnectedProject } from "../state/connectedProject";
import { homesToOffer } from "./homes";
import type { Home } from "./homes";
import "./rowButton.styles.css";

/**
 * U157: the light `move` word and the one list of Homes it opens. From a row the list is a popover below it (`popover`); from the Drawer it is inline.
 * `choose` answers the failure message or `null`. The Drawer keeps the list open on a failure and shows it inside; a row closes first and reports it on the row.
 */
export const HomeMove = (props: {
  id: number;
  home: string | null;
  popover?: boolean;
  class?: string;
  onOpenChange?: (open: boolean) => void;
  choose: (home: string | null) => Promise<string | null>;
}) => {
  const { rail } = useConnectedProject();
  const [open, setOpen] = createSignal(false);
  const [failure, setFailure] = createSignal<string | null>(null);
  const [word, setWord] = createSignal<HTMLButtonElement>();
  const [wrap, setWrap] = createSignal<HTMLSpanElement>();

  createEffect(() => props.onOpenChange?.(open()));

  const close = () => {
    // Focus first: the row unmounts this word once the list is shut and nothing else holds focus.
    word()?.focus();
    setOpen(false);
    setFailure(null);
  };

  const pick = async (offer: Home) => {
    if (props.popover) close();

    const message = await props.choose(offer.home);

    if (props.popover) return;

    if (message === null) close();
    else setFailure(message);
  };

  onMount(() => {
    // The Drawer closes on Esc at the document; the list answers first.
    const onKey = (key: KeyboardEvent) => {
      if (key.key !== "Escape" || !open()) return;

      key.stopPropagation();

      close();
    };

    const outside = (click: MouseEvent) => {
      if (open() && click.target instanceof Node && !wrap()?.contains(click.target)) close();
    };

    wrap()?.addEventListener("keydown", onKey);
    document.addEventListener("click", outside);
    onCleanup(() => document.removeEventListener("click", outside));
  });

  return (
    <span ref={setWrap} class="todo-move-wrap">
      <button
        type="button"
        class={`word light todo-move ${props.class ?? ""}`}
        aria-label={`move #${props.id}`}
        aria-expanded={open()}
        ref={setWord}
        onClick={() => {
          setFailure(null);
          setOpen(!open());
        }}
      >
        move
      </button>
      <Show when={open()}>
        <div class="todo-homes" classList={{ "todo-homes-popover": props.popover }} role="group" aria-label={`homes for #${props.id}`}>
          <For each={homesToOffer(rail.nodes, props.home)}>
            {(offer) => (
              <button type="button" class="word todo-home-offer" onClick={() => void pick(offer)}>
                {offer.name}
              </button>
            )}
          </For>
          <Show when={failure()}>{(message) => <ErrorLine message={message()} />}</Show>
        </div>
      </Show>
    </span>
  );
};
