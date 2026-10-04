import Check from "lucide-solid/icons/check";
import ChevronDown from "lucide-solid/icons/chevron-down";
import ChevronRight from "lucide-solid/icons/chevron-right";
import Circle from "lucide-solid/icons/circle";
import Diamond from "lucide-solid/icons/diamond";
import DiamondPlus from "lucide-solid/icons/diamond-plus";
import Dot from "lucide-solid/icons/dot";
import Pause from "lucide-solid/icons/pause";
import Plus from "lucide-solid/icons/plus";
import X from "lucide-solid/icons/x";
import { Dynamic } from "solid-js/web";
import "./styles.css";

const icons = { check: Check, down: ChevronDown, right: ChevronRight, circle: Circle, diamond: Diamond, owned: DiamondPlus, dot: Dot, pause: Pause, plus: Plus, x: X };

export const Icon = (props: { name: keyof typeof icons; filled?: boolean }) => (
  <Dynamic component={icons[props.name]} class="icon" aria-hidden="true" fill={props.filled ? "currentColor" : "none"} />
);
