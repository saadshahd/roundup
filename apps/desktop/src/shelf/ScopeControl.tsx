import { copy } from "../copy";
import ListFilter from "lucide-solid/icons/list-filter";
import { IconButton } from "../ui/IconButton";

/** U470: the shared Shelf scope is one icon button; its name says what the next press shows. */
export const ScopeControl = (props: { showAll: boolean; onChange: (showAll: boolean) => void }) => (
  <IconButton
    class="scope-control"
    aria-pressed={props.showAll}
    aria-label={props.showAll ? copy.showOnlyThisWorkstream : copy.showAllWorkstreams}
    onClick={() => props.onChange(!props.showAll)}
  >
    <ListFilter class="icon" aria-hidden="true" width={16} height={16} stroke-width={1.75} />
  </IconButton>
);
