import { createMemo, onCleanup, Show } from "solid-js";
import { useConnectedProject } from "../state/connectedProject";
import { attentionCount, nextAttention } from "./attention";

/** `<n> need you` in the header, and `⌘J`: both select the next Agent that needs the user. */
export const AttentionChip = () => {
  const { rail } = useConnectedProject();

  const count = createMemo(() => attentionCount(rail.nodes));

  const jump = () => {
    const next = nextAttention(rail.nodes, rail.selected());

    if (next) rail.select(next.id);
  };

  const onKey = (key: KeyboardEvent) => {
    if (key.metaKey && key.key === "j") jump();
  };

  document.addEventListener("keydown", onKey);
  onCleanup(() => document.removeEventListener("keydown", onKey));

  return (
    <Show when={count() > 0}>
      <>
        {"   "}
        <button class="word" onClick={jump}>{`${count()} need you`}</button>
      </>
    </Show>
  );
};
