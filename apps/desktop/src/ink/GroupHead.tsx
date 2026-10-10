import type { JSX } from "solid-js";
import { useConnectedProject } from "../state/connectedProject";
import { Button } from "./Button";
import { Icon } from "./Icon";

/** The id a Shelf list's collapse is saved under, beside the Workstream ids (U100). */
export type ShelfGroup = "shelf:todos" | "shelf:pads";

/** U172: the one head of a collapsible group: a mark, the label, a hairline rule and `<shown>/<total>`; `children` trail it (the add button). */
export const GroupHead = (props: { group: ShelfGroup; label: string; shown: number; total: number; children?: JSX.Element }) => {
  const { rail } = useConnectedProject();
  const closed = () => rail.collapsed().has(props.group);

  return (
    <div class="group-head" data-group={props.group}>
      <Button kind="quiet" class="word group-toggle" aria-expanded={!closed()} onClick={() => rail.toggleCollapsed(props.group)}>
        <Icon name={closed() ? "right" : "down"} />
        <span class="group-label">{props.label}</span>
      </Button>
      <span class="group-rule" aria-hidden="true" />
      <span class="group-count light" title={`${props.shown} shown of ${props.total}`}>
        {props.shown}/{props.total}
      </span>
      {props.children}
    </div>
  );
};
