# Motion

Terms from the animation vocabulary. The more often a change is seen, the shorter (or absent) its animation. Check D9 (`docs/design-system.md`) measures each duration against this table.

| Trigger | Term | Duration / easing | Notes |
|---|---|---|---|
| Inbox, Todo detail, Pad, history open | Slide in from the right | ~180 ms, ease-out | Exit reverses. Never resizes the terminal (spatial consistency). |
| Rail live-line text change | Crossfade | ~120 ms | No spinner (purposeful animation). |
| Status glyph change | none (instant) | 0 | Seen constantly. |
| Done Agents fold | Accordion / collapse | ~150 ms, ease-out | |
| New row | Layout animation | ~150 ms | Siblings move; nothing snaps. |
| Drag in the rail | Drag to reorder | follows pointer | Others shift to make room; thin drop line, its left end sets depth; release is interruptible. |
| Needs-you arrival | Pulse | one cycle, ~400 ms | Then still. An ambient loop would break the calm rule. |
| Rejected Inbox action | Shake / wiggle | ~250 ms | On the row. |

**Reduced motion** (`prefers-reduced-motion`) turns off every row above except the instant state changes.
