import { createMemo, onCleanup, Show } from "solid-js";
import { useConnectedProject } from "../state/connectedProject";
import { attentionCount, nextAttention } from "./attention";
import "./attentionChip.styles.css";
import { Button } from "../ink/Button";

/** `<n> need you` in the header, and `⌘J`: both select the next Agent that needs the user. */
export const AttentionChip = () => {
  const { rail } = useConnectedProject();

  const count = createMemo(() => attentionCount(rail.nodes));

  const jump = () => {
    const next = nextAttention(rail.nodes, rail.selected());

    if (next) rail.select(next.id);
  };

  const onKey = (press: KeyboardEvent) => {
    if (!press.metaKey || press.ctrlKey || press.altKey || press.shiftKey) return;

    if (press.key.toLowerCase() !== "j") return;

    press.preventDefault();
    jump();
  };

  document.addEventListener("keydown", onKey);
  onCleanup(() => document.removeEventListener("keydown", onKey));

  return (
    <Show when={count() > 0}>
      <>
        {"   "}
        <Button kind="quiet" class="word ink attention-chip" title="⌘J" onClick={jump}>{`${count()} need you`}</Button>
      </>
    </Show>
  );
};
