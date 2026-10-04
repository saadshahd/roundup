import type { Actor } from "@contracts/Actor";
import { Icon } from "./Icon";

export const OwnerMark = (props: { owner: Actor; decorative?: boolean }) => (
  <span role={props.decorative ? undefined : "img"} aria-label={props.decorative ? undefined : `${props.owner.kind} owned`}>
    <Icon name={props.owner.kind === "user" ? "diamond" : "owned"} />
  </span>
);
