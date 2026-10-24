# Design system: quiet native

How the App looks. `docs/wireframes.md` says what each screen holds and where; this file says how it is drawn. Where the two disagree on look, this file wins: it replaces the wireframes' "ink is the only colour" and "no borders" rules (the Glyph set, the visibility rule and the layout stay).

The direction: a Mac app that stays out of the way. System sans for the chrome, mono only where text is code. Soft surfaces, small coloured Glyphs, nothing moves unless the user acted or an Agent finished a Todo the user can see (U3010c, U3770), or the Workstream timeline's window slides with time while an Agent it shows runs (U3768e, U3768f); no shimmer or pulse. Light and dark.

Every rule below is a check with an id. A check passes or fails; it has no score. The same checks run twice: as `vitest` tests in `just check` (the gate), and by the Design critic on the real rendering (advisory). One module measures them for both (U137).

## Six rules

| # | Rule | Checks |
|---|---|---|
| 1 | Every look value is a Token; no other value is drawn. | D1, D2 |
| 2 | Hue is rare and means one thing: a Kind, or the one accent. | D3, D4 |
| 3 | Anything clickable looks clickable and answers at once. | D6, D7 |
| 4 | A check passes or fails on measured values, on screens the Builder did not tune for. | D1 to D16, the held-out list |
| 5 | A visual change is judged against the build before it, not against taste. | the baseline protocol |
| 6 | A word shown on every row appears only when it differs from what the reader expects. | U189 |
| 7 | Every word the App writes is in sentence case. | D15 |

## Tokens

`--text-selection: Highlight` is the platform selected-text background (CSS UI system colour, macOS NSColor.selectedTextBackgroundColor). Browser/editor selections resolve this Token in both schemes and increased contrast. U3517c's Terminal cells instead use their pinned Ghostty/Tomorrow selection Tokens. Selected text uses `--text-selection-ink: HighlightText`, the matching platform foreground, for legibility, including ANSI-coloured output and links; row/tab selection retains `--selected`. Source: https://www.w3.org/TR/css-color-4/#css-system-colors and https://developer.apple.com/documentation/appkit/nscolor/selectedtextbackgroundcolor.

All Tokens live in one file, `apps/desktop/src/tokens.css`. No other stylesheet, and no inline style, holds a colour, font size, radius, shadow, duration or easing literal; it reads `var(--…)`. Spacing uses the space Tokens, and `0`.

Starting values. The thresholds in the checks are fixed; a value that fails a threshold is changed, never the threshold.

| Token | Light | Dark | Use |
|---|---|---|---|
| `--ground` | `#ffffff` | `#1c1c1e` | the pane and the Drawer |
| `--sunken` | `#f5f5f7` | `#141416` | the Rail and the Shelf |
| `--hover` | `rgba(0,0,0,.04)` | `rgba(255,255,255,.06)` | a row or button under the pointer |
| `--selected` | `rgba(0,0,0,.05)` | `rgba(255,255,255,.10)` | the selected row |
| `--selected-hover` | `rgba(0,0,0,.07)` | `rgba(255,255,255,.13)` | the selected row under the pointer |
| `--scrim` | `rgba(0,0,0,.40)` | same | darkens the centre beneath an overlay Shelf |
| `--text` | `#1d1d1f` | `#f5f5f7` | body text |
| `--grey` | `#68686d` | `#a1a1a6` | secondary text, Glyph tone of `blocked`, `idle`, `done` |
| `--cache-write` | `#b4b4b9` | `#636368` | Measurement cache-write segment and swatch |
| `--disabled` | `#6f6f74` | `#8e8e93` | the label of a button that cannot act (4.5:1 on `--ground` and `--sunken`) |
| `--accent` | `#0a60d8` | `#4d9bff` | Glyph tone of `working`, the primary action |
| `--amber` | `#975900` | `#f0a030` | `needs-you` |
| `--red` | `#c4262e` | `#ff6b6b` | `error` |
| `--hairline` | `rgba(0,0,0,.10)` | `rgba(255,255,255,.12)` | the one permitted border |
| `--scrollbar-thumb` | `rgba(0,0,0,.30)` | `rgba(255,255,255,.30)` | the thumb of every scrollbar, shown only while the pointer is over its scrolling element (U3747) |
| `--shadow-drawer` | `0 2px 8px rgba(0,0,0,.14)` | `0 2px 8px rgba(0,0,0,.24)` | the Drawer, floating menus and the Terminal bubble (U523) |
| `--shadow-shelf` | `0 0 24px rgba(0,0,0,.10)` | `0 0 24px rgba(0,0,0,.45)` | the open Shelf's leading edge (U371) |
| `--shadow-scroll` | `inset 0 8px 8px -8px rgba(0,0,0,.18)` | `inset 0 8px 8px -8px rgba(0,0,0,.6)` | a Shelf list's clipped edge (U371) and the Rail tree's top edge beneath the pinned Overview band (U2716a) |
| `--centre-min-width` | `388px` | same | the centre floor and Shelf reservation threshold (U371, U389) |
| `--shelf-width` | `320px` | same | the open Shelf's width (U371; #622 makes it resizable) |
| `--text-pad-subheading` / `--text-pad-heading` | `20px` / `24px` | same | rendered and formatted Pad headings |
| `--radius-row` / `--radius-control` / `--radius-drawer` | `6px` / `6px` / `12px` | same | selection band, buttons, Drawer, Terminal bubble (U523) |
| `--easing-out` | `cubic-bezier(0.32, 0.72, 0, 1)` | same | every eased motion (`docs/motion.md`, U2579a) |
| `--duration-drawer` | `180ms` | same | the Drawer's slide in and out (`docs/motion.md`, ~180 ms) |
| `--duration-toast` | `150ms` | same | a Toast's enter and exit (`docs/motion.md`, ~150 ms, U414) |
| `--duration-todo` | `150ms` | same | the Todo fold and row slide (`docs/motion.md`, ~150 ms, U3010c) |
| `--ease-todo` / `--ease-pop` | `cubic-bezier(0.22, 1, 0.36, 1)` / `cubic-bezier(0.34, 1.56, 0.64, 1)` | same | a finishing Todo's tick, strike, bar and count / the ring's overshoot (`docs/motion.md`, U3771) |
| `--duration-tick` / `--delay-tick` | `420ms` / `160ms` | same | the tick draw (U3771) |
| `--duration-ring` | `560ms` | same | the ring pop (U3771) |
| `--duration-strike` / `--delay-strike` | `620ms` / `220ms` | same | the label strike (U3771) |
| `--strike-thickness` / `--strike-offset` / `--stroke-tick` | `1px` / `55%` / `1.5` | same | the label strike line's thickness and height in the label, and the tick mark's stroke width (U3769) |
| `--duration-bar` / `--duration-count` | `1100ms` / `560ms` | same | the progress bar fill / the count roll (U3771) |
| `--duration-crossfade` | `120ms` | same | a base interval (`docs/motion.md`, 120 ms); the working arc uses eight intervals (U446). U105 periphery dimming is retired |


Type: `--font-ui` is `-apple-system, BlinkMacSystemFont, "SF Pro Text", system-ui, sans-serif`; `--font-mono` is `ui-monospace, "SF Mono", Menlo, monospace`. The steps are `--text-caption` 11, `--text-small` 12, `--text-body` 13, `--text-row` 14 (every Rail row's name), `--text-title` 15, `--text-pad-subheading` 20 and `--text-pad-heading` 24 px. Weight is 400, 500 or 600. Tracking is `0` at body and below, `-0.01em` at title (apple-design: tracking follows size).

The App inherits `--font-ui`; code, paths, diffs, ids and token counts select `--font-mono`; Terminal output selects `--font-terminal`. `--mono` aliases `--font-mono` in the Token file. Static interface typography uses the listed type steps. U102’s user-adjustable Terminal font starts at `--text-terminal` and changes in 1 px increments within `--text-terminal-min` and `--text-terminal-max`; this scale applies only to the Terminal, not the Rail or other interface text.

Pad prose uses `--text-title` (15 px). Its `--text-pad-subheading` (20 px) and `--text-pad-heading` (24 px) steps apply only to rendered and formatted Pad headings, in both colour schemes; source and code stay mono at `--text-body`. A Pad's reading and editing views use the same steps.

The reserved message line (`--line`) is 24 px in both schemes, including its actions. It stays mounted when empty; compact surfaces reserve an overlaid edge instead.

Space: `--space-1` to `--space-6` are 4, 8, 12, 16, 24, 32 px. A Rail row is 28 px high. U2716a specifies a future pinned 40 px Overview band above the scrolling Rail tree, with `--space-3` inline inset, a 24 px icon tile and `--text-row` at weight 400; its count rides beside the name. `--measure` is `56rem`, the Thread's column width (D11).

Mono is for the Terminal, paths, ids and code. Every other word is `--font-ui`.

## Colour

Kind has two carriers, a Glyph shape and a tone. The shape is enough on its own: colour never carries a Kind alone. Ink (hue only, never a weight change) stays what `GLOSSARY.md` says: only `needs-you` and `error`.

| Kind | Glyph | Tone | Ink |
|---|---|---|---|
| `error` | `✕` | `--red` | yes |
| `needs-you` | `●` | `--amber` | yes |
| `working` | `○` | `--accent` | no |
| `blocked` | `⏸` | `--grey` | no |
| `idle` | `·` | `--grey` | no |
| `done` | `✓` | `--grey` | no |

Three hues exist in the whole App: `--red`, `--amber`, `--accent`. Surfaces are neutral. A screen with no `error` and no `needs-you` shows no red and no amber.

## Click targets

**Ghost, icon and disclosure buttons (U510)**: quiet, add, row-action and icon buttons draw no fill, border, radius or inline padding on hover or press. Hover moves the ink from `--grey` to `--text`; press dims it to `--ink-pressed`; a label and its chevron sit `--space-1` apart. Focus never draws a ring (U3528). A toggled icon keeps its persistent selected surface with no hover or press fill. A control resting on a selected wash (the Rail's selected-row Add Agent, a selected tab's ×) rests at `--ink-rest-selected`, a step below hover's `--text`. U2716a specifies the Project root as a future pinned Overview band: its hover wash, selected ground and icon tile follow that scenario, with U362's Todo drop highlight preserved. Tabs and a row's pads count (`display: none` while the pointer is near its row) keep their existing behaviour; `apps/desktop/src/app/u510_ink_feedback.test.ts` inventories every other caller on `tree-40`, `thread-report`, `tabs-8` and `door-stopped`.

A click target is any element with a click, press or key handler, a `button`, `a` or `[role=button|row|tab]`, or `cursor: pointer`. Each has these states. Focus adds no paint and pressing adds no background (U3528):

| State | Looks like | When |
|---|---|---|
| rest | its resting surface; a button also has a label in `--text` or `--grey` and a hit height of at least 24 px | always |
| hover | `--hover` surface, cursor `pointer` | pointer over it |
| pressed | unchanged background; existing ink feedback only | from `pointerdown`, before `pointerup` |
| focus | no outline or focus shadow; fields keep their caret | every input path (U3528) |
| disabled | `--disabled` text, cursor `default`, `aria-disabled`, no hover or pressed change | when it cannot act |

A `textarea` is a text field, not a pressed surface: D6 and D7 skip it, because it answers with a text cursor while focused (U3528) instead of a hover surface, a pointer and a pressed style.

A disabled button stays focusable and keeps `aria-disabled="true"`, never the `disabled` attribute, so its reason can be read as its `title`; its handler does nothing while it is set.

A word such as `stop`, `close` or `promote` is a button, not text: it has ink feedback and a 24 px hit height even though it rests as plain `--grey`.

### Todo plans and dependency groups (U3808–U3811)

An Agent's latest eligible recorded Hand-off reads `now` beside its title and marks its treeitem as the current step. Unknown Hand-offs show no marker; no Kind or list position guesses it. Done remains the native tick's accepted value.

The first blocker in an owner group defines nesting even when done. A parent keeps its descendants, their own progress bar and a separate disclosure; its title strikes only when every descendant is done. Locked rows name all open blockers. Neither disclosure nor progress changes Todo facts. A visible final-child completion finishes its tick/strike and bar before folding; only the first newly unblocked successor per owner unfolds. Mount, scope changes and hidden completions settle without replay; reduced motion settles immediately.

### Button kinds

Every button of the Rail, Shelf, Drawer, Thread and centre is one of five kinds and carries `data-button` naming it. `apps/desktop/src/buttons.css` is the one stylesheet that draws a button: one rule per kind, from Tokens, and no other stylesheet sets a `background`, `color`, `border-radius` or `padding` on a `button` (a component sets layout, and the hooks `--button-surface`, `--button-inline`, `--button-lead` and `--button-radius`). `data-feedback="ink"` swaps hover and press surfaces for ink only (U523). A button inside a row keeps its kind. A row is not a button: a `treeitem` row of the Rail, a Shelf row and a Todo's title take no `data-button` and are drawn by U172.

| Kind | Rest | Use |
|---|---|---|
| primary | `--accent` label | the one action of a state |
| quiet | `--grey` label (`--text` on hover), no surface | a word that acts; a link in a caption line stays in the text flow |
| add | a `+` mark and a label, quiet | trails a list or group, or, as the Rail's `+ Workstream` (U391), fills the Rail's bottom edge |
| row action | a word or mark in `--grey`, shown on a row's hover, focus or selection, always reachable by keyboard | `Move`, `Complete`, `Start supervisor` |
| icon button | a 16 px mark in a 24 px square, with an `aria-label` | Close, Remove, Collapse |

An icon toggle with `aria-pressed="true"` uses `--text` on `--selected`, `--selected-hover` on hover, and keeps that background from `pointerdown` (U3528). Every kind has the five states above. Pressed adds no background; the label tone of disabled is `--disabled`.

## Surfaces and depth

The Rail and the Shelf sit on `--sunken`; the pane and the Drawer on `--ground`. Regions are told apart by that change of ground and by space, and by one hairline on the Rail's trailing edge and one on the Shelf's leading edge, each a Resize edge the user drags or keys to change the width (U389, U390). The only borders allowed are `1px solid var(--hairline)`, on the Rail's trailing edge and the Shelf's leading edge, and the Drawer's leading edge (U134), plus the 1 px `--hairline` U172 draws between the Rail's blocks (a line above each Workstream row but the first) and as the rule of a group head. Only the Drawer, a Terminal bubble, floating menus and the open Shelf cast a shadow (`--shadow-drawer`, `--shadow-shelf`), and clipped Shelf lists (U371) and the Rail tree beneath its pinned Overview band (U2716a) show `--shadow-scroll`. The Shelf spans the window's trailing edge from top to bottom, below the Drawer (U369 to U371). It reserves its width when the window less the Rail and Shelf leaves at least `--centre-min-width` (388 CSS px); below that floor it overlays the unchanged centre with a `--scrim` scrim. The Drawer owns the centre reservation while open. The Shelf has one `--hairline` on its leading edge instead of a top edge; its two tabs are quiet buttons (U373), and its shared scope toggle is an icon button beside the hide arrow (U470). The open and close slide and overlay scrim opacity use the Drawer's duration and ease-out curve (`docs/motion.md`); the scrim stays mounted through closing and reverses from its current opacity. A translucent surface (`backdrop-filter`) is never placed over another (apple-design: never stack light materials), and the terminal never sits under one (U5), except the background Agent pane beneath an activated, focused Terminal Bubble (U2742a); a bubble is opaque `--ground` with `--radius-drawer`, `--shadow-drawer` and `--space-3` padding (U523); its bar is a macOS panel bar 28 px high, with a close dot and a minimise dot at the left, the name centred at 13 px, no box on any of them and no status (U2307c, U3704); the open bubble sits in the browser top layer, above every `z-index` (U2307b).

Floating layers (U3295a): one order in `tokens.css`, lowest first: `--layer-drawer`, `--layer-popover` (Popover, the ask surface; above the Drawer it opens from), `--layer-dialog` (Dialog, the switcher), `--layer-menu` (a Menu or Select, also when opened from inside a Dialog), `--layer-tooltip` (a label on any control, Drawer and Dialog ones included), `--layer-toast`. A floating surface takes its `z-index` from one of them and sets no number.

Three user settings change the look. Each has a rule in `tokens.css`:

- `prefers-color-scheme: dark` uses the dark column.
- `prefers-contrast: more` makes every `--hairline` 40% opaque and every `--grey` text `--text`.
- `prefers-reduced-transparency: reduce` removes every `backdrop-filter`, on elements and `::backdrop`, retaining Bubble dimming (U2742a).

## The checks

Each check is measured from computed style and the DOM of the running App, never from pixels, on every visible element of the screen, in light and in dark. A check that cannot measure fails; it never passes by default.

| Id | Passes when | Scenario |
|---|---|---|
| D1 | every computed colour, background, border-color, shadow, radius and duration is the value of a Token | U130 |
| D2 | every static interface font size is a type step; U102 Terminal sizing uses its bounded Token-defined scale; every margin, padding and gap is `0` or a space step, except U2541 `kbd.key-chip`: zero vertical padding and 5 px on each side matching `--key-chip-padding` (D1 still requires Token-only declarations). This adds no general space step. | U131 |
| D3 | every colour whose saturation exceeds 15% is `--red`, `--amber` or `--accent`; with no `error` or `needs-you` row on screen, no red or amber is drawn | U132 |
| D4 | every Rail, Shelf and Drawer item row (not editor menu categories or commands) shows one of the six Glyphs for its Kind; no Kind is drawn by colour only | U132 |
| D5 | text has at least 4.5:1 against its own ground, and a Glyph tone 3:1 (`done` is exempt as U4 says), in light and in dark | U4, U135 |
| D6 | every click target except a `textarea` retains hover feedback, keyboard reachability and a hit area at least 24 px high; no focus ring (U3528) | U133, U3528 |
| D7 | every click target adds no pressed background at `pointerdown`; ink feedback may remain (U3528) | U133 |
| D8 | no border is wider than 1 px or any colour but `--hairline`; only the Drawer, the Terminal bubble, the Shelf, the Terminal's control overlay (U375) and floating menus have a shadow (menus use `--shadow-drawer` on their surface only), and a list's scroll shadow is `--shadow-scroll`; no `backdrop-filter` element contains another | U134, U371 |
| D9 | each duration is within 25% of `docs/motion.md`, and under `prefers-reduced-motion` all but the instant changes are 0; duration membership alone proves neither movement nor smoothness (U3010a, `scenarios/ui-motion.md`) | U5, U135 |
| D10 | D1 to D9 hold in both colour schemes, in `prefers-contrast: more`, and at 1280 by 800 | U135 |
| D11 | in the Thread, the column is at most `--measure` (`56rem`) wide, no rendered line holds more than 120 characters, and every row's text starts at one x within 1 px, at 1280 by 800 | U197 |
| D12 | in each of the Shelf's `todos` and `pads` lists, after U3708's tree indent, the Glyphs' left x are within 1 px of one another; first words align within 1 px among tick-only rows and among rows with a Glyph column (U3726); every later line starts at its own title's left x within 1 px | U360, U3726 |
| D13 | in the Shelf, every text node is `--font-ui` unless it is an id, a path or code | U360 |
| D14 | in the Shelf, no row has a fill, border or shadow at rest; the gaps between rows, from a head to its first row and between the two lists are space steps, equal across the lists; no title or name shows more than two lines or overflows its row | U360 |
| D15 | all App-authored text and accessible names come from the App copy catalogue; its entries and formatter branches use sentence case and `GLOSSARY.md` user labels, with explicit section-head, acronym and authored-value exemptions | U471, U472 |
| D16 | in the Thread, every text node is `--font-ui` unless it is an id, a path or code, and no Agent name wraps | U425 |
| D17 | A tooltip sits against the element under the pointer that owns it. | U2749 |

## Text case

Sentence case: the first word capitalised, the rest as in prose (`Start supervisor`, `Send`, `No checkpoint yet`, `Clarifying: What is this Workstream for?`; a word after a colon is capitalised like a first word). A `GLOSSARY.md` term without an explicit user-label mapping keeps its capital inside a phrase (`Workstream`, `Agent`, `Pad`). Exempt: section heads, which stay in spaced capitals (#1097); names, ids, paths, code and text the user or an Agent wrote, as written (a Workstream named `workstream` stays lower case); Terminal text and Agents' output. An add button reads `+ ` then the term (`+ Workstream`), with no `new`. This replaces `docs/wireframes.md`'s lower-case click-target rule.

All App-authored text comes from `apps/desktop/src/copy.ts`, including accessible names, errors, statuses, hints, placeholders, tooltips, section heads and generated prose. Identifiers and RPC values keep code terms; App prose uses supervisor, work items and summary for Door, Shelf and Handoff. Other domain terms retain their user spelling. One catalogue-wide case observer replaces per-folder case checks while preserving their interaction and authored-text assertions.

## Held-out screens

A check is public; the screens it runs on are not all the Builder's. The critic also runs the held-out screens in `.agents/data/harness.md`, so a PR cannot tune one screen to pass.

## Ideas

The critic's report ends with an `ideas` list of at most three things that look wrong and no check covers. An idea is never a Todo and never a gate. The Builder run reads the ideas each batch. An idea that comes back three times becomes a check (`.agents/builder.md`).

## Baseline protocol

The critic captures `origin/main` and the PR head on the same seed, viewport, colour scheme and injected clock (`.agents/data/harness.md`, Captures); `loop/rules.sh delta` prints each check as `fixed`, `regressed`, `still-failing` or `still-passing`. A `regressed` check is a Todo of class `ux-checklist`, and a reject if it is in `just check`. A `still-failing` check is queued; it is a defect only once a scenario names it.


### U441 scoped header and tab Tokens

Root Tokens and painted surfaces outside the header stay unchanged. Backing values in `tokens.css` are assigned to the existing Token names at each surface; callers read only those names.

| Surface | Token | Light | Dark |
|---|---|---|---|
| header backdrop and bare header controls | `--ground` | root `--sunken`: `#f5f5f7` | root `--ground`: `#1c1c1e` |
| unselected tab base | `--sunken` | root `#f5f5f7` | root `#141416` |
| tab selected base | `--selected` | `#ffffff` | `#2b2b2d` |
| tab selected under pointer | `--selected-hover` | `#fafafa` | `#323234` |
| header rule and tab borders | `--hairline` | opaque `#68686d` | opaque `#8e8e93` |
| header and tabs | `--text` | root `#1d1d1f` | root `#f5f5f7` |
| bare header controls | `--grey` / `--disabled` | root `#68686d` / `#6f6f74` | root `#a1a1a6` / `#8e8e93` |
| unselected tabs | `--grey` | root `#68686d` | root `#a1a1a6` |
| tab controls | `--disabled` | `--text` | `--text` |
| header and tab focus | no paint (U3528) | none | none |
| header Ink and Tab marks, including overflow | `--red` / `--amber` | root `#c4262e` / `#975900` | root `#ff6b6b` / `#f0a030` |
| bare header controls | `--hover` | root values | root values |
| unselected tab and its × | `--hover` | root values | root values |
| × inside selected hovered box | `--hover` | root `rgba(0,0,0,.04)` | root `rgba(255,255,255,.06)` |
| selected tab controls | selected tab ground | unchanged | unchanged |

Paint order: opaque header, opaque tab base, one unselected hover wash (selected hover replaces its base), then the close control’s local wash and Ink. The name contributes no second wash. Press keeps the resting background; the selected name and × retain the Enter press through keyup or blur via the shared button pressed hook. Overflow uses unselected-tab values; the strip's edge caps keep their opaque rest surface and answer with ink only; rename uses its own tab ground and text. Tooltips, tab menus and close dialogs resolve the root Tokens. More contrast promotes each scoped grey to its local text and preserves the opaque hairline. D3 and D8 resolve their colour Tokens at the measured element; D1 accepts no caller colour literal.

The light header stays on root sunken. Borders carry ≥3:1 against the header and adjacent tab; selection uses a lighter ground and full ink, without a selected/header contrast threshold. Dark selected rest, hover and press stay no brighter than the corresponding selected Rail wash. Painted name buttons and rename fields carry exactly `--space-1` on both inline edges. The tab minimum includes those two insets outside U382’s eight-character text floor and the existing close target; the remaining box geometry belongs to U440.


### U2541 shortcut key chips

Hint line, Help and leader keys use the shared `KeyChip`, a plain `kbd` per symbol.
Labels remain `--grey`; chips use `--text`, `--font-ui`, `--text-small`, weight 500,
no border and a `--space-4` (16 px) height.

| Token | Light | Dark |
| --- | --- | --- |
| `--key-chip-fill` | `rgba(0, 0, 0, 0.06)` | `rgba(255, 255, 255, 0.08)` |
| `--radius-chip` | `4px` | same |
| `--key-chip-padding` | `5px` | same |

Increased contrast retains the fill and promotes labels to text. Whole hints drop from
the end when space runs out (U2230b). Keys have no motion (D6).


## Terminal Bubble focus and return

Scoped to Terminal Bubbles (U2742a–d, `scenarios/ui-bubble-panel.md`); other App surfaces keep their rules.

| Belief | Rule and pass/fail observer |
| --- | --- |
| Focus on one matter | Only a user-activated Bubble with focus inside has one centre-pane backdrop: `--scrim` dimming and `--blur-bubble-backdrop` (12px in both schemes). Foreground text, header and controls stay opaque and unfiltered. Return removes the backdrop in the same render; no transition or delay. A new unfocused Run neither dims nor steals focus. U2742a App behavior and paired rendered captures decide. |
| Large enough to work; visibly temporary | Fresh unsaved rectangles use, on each axis, `size = min(paneExtent, max(existingMinimum, paneExtent - 192))`, `position = (paneExtent - size) / 2`. Nominal outer inset is 96 CSS px (three `--space-6` steps); retain the 24-column/6-row minimum and clamping. Preserve all saved rectangles and user-requested fill. U2742b rendered captures at 1512×950 and 1280×600 decide. |
| The Agent is the main thing | Return focuses the same selected Agent emulator through `Screens`, without Rail selection change or a tab/header focus stop. U2742c/d App typing proofs decide. |
| One shortcut both ways | One non-repeated ⌘B switches Agent → last-used eligible Terminal → Agent; a terminal tab restores its own Terminal. No second action or animation completion. U2742c/d App browser tests decide. |
| Temporary view, continuing work | Preserve Terminal identity, command, screen/scroll, draft input and saved rectangle; switching opens/closes nothing and writes no chord. Close stays destructive and separate. U2742c/d call and emulator assertions decide. |

Use the current Ark Panel and manual popover top layer. `.bubble-frame::backdrop` uses the existing pane origin and extent with `pointer-events: none`, so outside dismissal still reaches its target once (U2307a). Derive activation from shown/focus state; no persisted mode. Filters never nest. Reduced transparency clears filters on both elements and backdrops, retaining dimming. Only the background Agent terminal receives this blur exception; the Bubble keeps its opaque ground, radius, shadow, bar and internal inset (U523/U2307c). U2742's rendered observer checks layer bounds and foreground clarity; its recorded Mac observation checks actual chord delivery, direct focus return and WebKit rendering.

### U3512 shared floating surface

Native confirmations use the system's appearance. Every remaining web modal, anchored panel/menu and Terminal Bubble shares the recipe in `tokens.css`: opaque surface, 12 px radius, one hairline, one `--shadow-drawer`. Screen CSS owns content layout only. D8 allows this depth on `.app-dialog`, `.app-menu`, `.app-popover` and `.app-select-list` as well as its existing floating surfaces. Every `box-shadow` application, including scroll cues and resets, lives in `tokens.css`; Stylelint rejects the property elsewhere.

| Token | Light | Dark | Role |
| --- | --- | --- | --- |
| `--surface` | `#f5f5f7` | `#242426` | Opaque floating surface |
| `--button-neutral` | `#ededf0` | `#343438` | Secondary/action button fill |
| `--button-accent-ink` | `#ffffff` | `#101014` | Label on accent fill |
| `--radius-pill` | `999px` | same | Floating-surface action buttons |

The existing primary kind uses `--accent` and `--button-accent-ink`; the existing quiet kind uses `--button-neutral` and `--text`. Both use `--radius-pill`, 24 px minimum targets and space Tokens. Hover changes ink only. Menu items retain their menu semantics. This scoped exception supersedes the ink-only/no-control-radius rule within these floating surfaces; other controls keep it. The shared backdrop uses the existing `--scrim` in both schemes and never blurs. The Terminal Bubble retains its existing background-Agent treatment.


### Terminal Tokens (U3517)

Terminal defaults use bundled JetBrains Mono 2.304 (regular, bold, italic and bold
italic, OFL in `apps/desktop/public/fonts/jetbrains-mono/`). `--font-terminal` is
`"JetBrains Mono", ui-monospace, "SF Mono", Menlo, monospace`. Only Terminal content
uses it; the rest of the App retains its existing type.

The emulator, App Settings and Project zoom reset share `--text-terminal` (14 CSS
px). `--line-terminal` is xterm's unitless line-height multiplier (1.06) and
`--letter-terminal` is extra cell tracking (0 CSS px). These are initial calibrated
web values, starting from Ghostty's documented macOS 13 pt default, not a conversion
of native points to CSS pixels. Rendered cell measurements and remaining native
comparison are recorded in `docs/terminal-type.md`.

`--terminal-background`, `--terminal-foreground`, `--terminal-cursor`,
`--terminal-cursor-text`, `--terminal-selection`, `--terminal-selection-text`, and
`--terminal-{black,red,green,yellow,blue,magenta,cyan,white}` with their
`bright-` variants hold every emulator colour. CSS viewport/inset and X17 colour
replies consume the same columns. Dark uses Ghostty's pinned unconfigured palette;
Light uses its shipped Tomorrow companion (sources in `scenarios/ui-terminal-type.md`).
Default text and App controls retain D5; reference ANSI cells and program-authored
colours have U3517c's explicit exception. No colour is brightened to meet a Check.


## Design-system pass requirements (U130–U137)

Specification policy; runtime values above remain the baseline until implementation. The acceptance in `scenarios/ui-visual.md`, heading “Design-system pass acceptance (U130–U137)”, adds duties to D4, D7, D8, D9 and D10 without removing the existing Checks.

| Check | Additional requirement |
| --- | --- |
| D4 | Every enabled control answers a press with Token colour, opacity or scale by the first subsequent frame. A control unfilled at rest gains no hover or pressed ground. |
| D7 | A selected, focused or dragged item row has exactly one changed cue: text colour or one selected ground; combined states reuse the cue. |
| D8 | Rail, Todo, Pad and Terminal-tab first text uses one local inset after tree indentation; text and Glyph optical baselines agree within 1 CSS px. |
| D9 | Stylelint rejects importance, unowned px/ms/s/colours, selectors over two compounds and child combinators; no existing-hit allowlist. Duration and reduced-motion checks remain. |
| D10 | Floating surfaces share contact + ambient shadow layers and a 0.5 CSS px edge ring, replacing both old shadow Tokens and every consumer. App palette uses Radix light/dark scales plus the system accent. Rail and Shelf use native vibrancy, opaque under reduced transparency; centre, Terminal pixels and floating panels stay opaque. |

D10’s edge ring is not a control outline or focus ring. Native material is the sole new transparency exception for Rail/Shelf. No shader or WebGL background. Terminal ANSI remains U3517’s palette. Browser captures prove web rendering only; native material and launch cost require the implementation’s Mac observation. No new Token values, App seam names or native performance claims are established by this policy merge.
