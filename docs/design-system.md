# Design system: quiet native

How the App looks. `docs/wireframes.md` says what each screen holds and where; this file says how it is drawn. Where the two disagree on look, this file wins: it replaces the wireframes' "ink is the only colour" and "no borders" rules (the Glyph set, the visibility rule and the layout stay).

The direction: a Mac app that stays out of the way. System sans for the chrome, mono only where text is code. Soft surfaces, small coloured Glyphs, nothing that moves unless the user did something. Light and dark.

Every rule below is a check with an id. A check passes or fails; it has no score. The same checks run twice: as `vitest` tests in `just check` (the gate), and by the Design critic on the real rendering (advisory). One module measures them for both (U137).

## Five rules

| # | Rule | Checks |
|---|---|---|
| 1 | Every look value is a Token; no other value is drawn. | D1, D2 |
| 2 | Hue is rare and means one thing: a Kind, or the one accent. | D3, D4 |
| 3 | Anything clickable looks clickable and answers at once. | D6, D7 |
| 4 | A check passes or fails on measured values, on screens the Builder did not tune for. | D1 to D10, the held-out list |
| 5 | A visual change is judged against the build before it, not against taste. | the baseline protocol |

## Tokens

All Tokens live in one file, `apps/desktop/src/tokens.css`. No other stylesheet, and no inline style, holds a colour, font size, radius, shadow or duration literal; it reads `var(--…)`. Spacing uses the space Tokens, and `0`.

Starting values. The thresholds in the checks are fixed; a value that fails a threshold is changed, never the threshold.

| Token | Light | Dark | Use |
|---|---|---|---|
| `--ground` | `#ffffff` | `#1c1c1e` | the pane and the Drawer |
| `--sunken` | `#f5f5f7` | `#141416` | the Rail and the Shelf |
| `--hover` | `rgba(0,0,0,.04)` | `rgba(255,255,255,.06)` | a row or button under the pointer |
| `--selected` | `rgba(0,0,0,.05)` | `rgba(255,255,255,.10)` | the selected row |
| `--pressed` | `rgba(0,0,0,.10)` | `rgba(255,255,255,.16)` | a button while the pointer is down |
| `--text` | `#1d1d1f` | `#f5f5f7` | body text |
| `--grey` | `#68686d` | `#a1a1a6` | secondary text, Glyph tone of `blocked`, `idle`, `done` |
| `--disabled` | `#68686d` | `#8e8e93` | the label of a button that cannot act |
| `--accent` | `#0a60d8` | `#4d9bff` | focus ring, Glyph tone of `working`, the primary action |
| `--amber` | `#975900` | `#f0a030` | `needs-you` |
| `--red` | `#c4262e` | `#ff6b6b` | `error` |
| `--hairline` | `rgba(0,0,0,.10)` | `rgba(255,255,255,.12)` | the one permitted border |
| `--shadow-drawer` | `0 8px 32px rgba(0,0,0,.14)` | `0 8px 32px rgba(0,0,0,.5)` | the Drawer only |
| `--text-pad-subheading` / `--text-pad-heading` | `20px` / `24px` | same | rendered and formatted Pad headings |
| `--radius-row` / `--radius-control` / `--radius-drawer` | `6px` / `6px` / `12px` | same | selection band, buttons, Drawer |
| `--duration-drawer` | `180ms` | same | the Drawer's slide in and out (`docs/motion.md`, ~180 ms) |
| `--duration-crossfade` | `120ms` | same | a crossfade (`docs/motion.md`, ~120 ms): the periphery dimming while the Thread's input is typed in (U105) |

Type: `--font-ui` is `-apple-system, system-ui, sans-serif`; `--font-mono` is `ui-monospace, "SF Mono", Menlo, monospace`. The steps are `--text-caption` 11, `--text-small` 12, `--text-body` 13, `--text-title` 15, `--text-pad-subheading` 20 and `--text-pad-heading` 24 px. Weight is 400, 500 or 600. Tracking is `0` at body and below, `-0.01em` at title (apple-design: tracking follows size).

Static interface typography uses the listed type steps. U102’s user-adjustable Terminal font starts at `--text-body` and changes in 1 px increments within `--text-terminal-min` and `--text-terminal-max`; this scale applies only to the Terminal, not the Rail or other interface text.

Pad prose uses `--text-title` (15 px). Its `--text-pad-subheading` (20 px) and `--text-pad-heading` (24 px) steps apply only to rendered and formatted Pad headings, in both colour schemes; source and code stay mono at `--text-body`. A Pad's reading and editing views use the same steps.

Space: `--space-1` to `--space-6` are 4, 8, 12, 16, 24, 32 px. A Rail row is 28 px high.

Mono is for the Terminal, paths, ids and code. Every other word is `--font-ui`.

## Colour

Kind has two carriers, a Glyph shape and a tone. The shape is enough on its own: colour never carries a Kind alone. Ink (bold plus hue) stays what `GLOSSARY.md` says: only `needs-you` and `error`.

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

A click target is any element with a click, press or key handler, a `button`, `a` or `[role=button|row|tab]`, or `cursor: pointer`. Each has these states, and each state differs from the one before it in a computed colour, background or outline:

| State | Looks like | When |
|---|---|---|
| rest | its resting surface and its kind's label; a hit height of at least 24 px | always |
| hover | `--hover` surface, cursor `pointer` | pointer over it |
| pressed | `--pressed` surface, with no transition | from `pointerdown`, before `pointerup` |
| focus | 2 px `--accent` outline, offset 1 px | `:focus-visible` |
| disabled | `--disabled` text, cursor `default`, `aria-disabled`, no hover or pressed change | when it cannot act |

A row is not a button: a Rail row (`treeitem`), a Shelf row and a Todo's title take the hover, selected and focus looks of "The sidebar" below and no kind. Every other click target is one of five kinds, named by its `data-button` attribute and drawn by the one rule of that name in `apps/desktop/src/buttons.css`, which is the only stylesheet that sets a `background`, `color`, `border-radius` or `padding` on a `button`:

| `data-button` | Rest | Used for |
|---|---|---|
| `primary` | `--accent` label, weight 600 | the one action of a state: `start Door`, `new Workstream`, `send`, `complete` in a Todo's Drawer |
| `quiet` | `--grey` label, `--text` on hover, no surface at rest | every other word: `stop`, `export .md`, `retry`, a menu item |
| `add` | `--grey`, `--text-small`, a `+` mark and a label | trailing a list or group: `+ agent`, `+ terminal`, `+ new Workstream` |
| `row-action` | `--grey`, `--text-caption`, a word or mark | on a row, shown on its hover, focus or selection and always reachable by keyboard: `complete`, `start Door`, a Todo's home |
| `icon` | a 16 px mark in a 24 px square, `--grey`, with an `aria-label` | `close`, the chevron, `+` in a group head, `remove` on an attachment |

Pressed, focus and disabled are the same for the five: a disabled button stays focusable and keeps `aria-disabled`, never the `disabled` attribute, so its reason can be read as its `title`. A word such as `stop`, `close` or `promote` is a button, not text: it has the hover surface and a 24 px hit height even though it rests as plain `--grey`.

## The sidebar

The Rail and the Shelf follow one hierarchy (U172), after the reference the user named:

- A **Workstream row** reads a square badge of the Workstream's first two letters, its name in bold, a disclosure chevron and a quiet right-aligned value (its Agents that are not `done`).
- A **group** (the Shelf's `todos` and `pads`) has a head: a 16 px mark, a spaced uppercase label, a `--hairline` rule and a count `<shown>/<total>`; it collapses, and the collapse is kept across a restart (U100).
- An **item** reads its Kind Glyph, a bold title and, below, a grey description clamped to two lines. No key hint is drawn, since no chord addresses a single row.
- The **selected** row has one mark: a `--selected` band with a 2 px `--accent` bar on its leading edge. Hover is `--hover` and never reads as selected.
- One `--hairline` sits between each block of the Rail. Rows have at least `--space-2` of vertical and `--space-4` of horizontal padding.

## Surfaces and depth

The Rail and the Shelf sit on `--sunken`; the pane and the Drawer on `--ground`. Regions are told apart by that change of ground and by space, and by one hairline on the Rail's trailing edge and one on the Shelf's top edge. The only borders allowed are `1px solid var(--hairline)`, on the Rail's trailing edge, the Shelf's top edge, the Drawer's leading edge and between the blocks of the Rail (U134, U172). Only the Drawer casts a shadow. A translucent surface (`backdrop-filter`) is never placed over another (apple-design: never stack light materials), and the terminal never sits under one (U5).

Three user settings change the look. Each has a rule in `tokens.css`:

- `prefers-color-scheme: dark` uses the dark column.
- `prefers-contrast: more` makes every `--hairline` 40% opaque and every `--grey` and `--disabled` text `--text`.
- `prefers-reduced-transparency: reduce` removes every `backdrop-filter`.

## The checks

Each check is measured from computed style and the DOM of the running App, never from pixels, on every visible element of the screen, in light and in dark. A check that cannot measure fails; it never passes by default.

| Id | Passes when | Scenario |
|---|---|---|
| D1 | every computed colour, background, border-color, shadow, radius and duration is the value of a Token | U130 |
| D2 | every static interface font size is a type step; U102 Terminal sizing uses its bounded Token-defined scale; every margin, padding and gap is `0` or a space step | U131 |
| D3 | every colour whose saturation exceeds 15% is `--red`, `--amber` or `--accent`; with no `error` or `needs-you` row on screen, no red or amber is drawn | U132 |
| D4 | every Rail, Shelf and Drawer row shows one of the six Glyphs for its Kind; no Kind is drawn by colour only | U132 |
| D5 | text has at least 4.5:1 against its own ground, and a Glyph tone and a focus ring 3:1 (`done` is exempt as U4 says), in light and in dark | U4, U135 |
| D6 | every click target shows its rest, hover and focus states from the table above, and its hit area is at least 24 px high | U133 |
| D7 | every click target's pressed style differs from hover, appears at `pointerdown` and has a transition of `0s` | U133 |
| D8 | no border is wider than 1 px or any colour but `--hairline`; only the Drawer has a shadow; no `backdrop-filter` element contains another | U134 |
| D9 | each duration is within 25% of `docs/motion.md`, and under `prefers-reduced-motion` all but the instant changes are 0 | U5, U135 |
| D10 | D1 to D9 hold in both colour schemes, in `prefers-contrast: more`, and at 700 by 800 | U135 |

## Held-out screens

A check is public; the screens it runs on are not all the Builder's. The critic also runs the held-out screens in `.agents/data/harness.md`, so a PR cannot tune one screen to pass.

## Ideas

The critic's report ends with an `ideas` list of at most three things that look wrong and no check covers. An idea is never a Todo and never a gate. The Builder run reads the ideas each batch. An idea that comes back three times becomes a check (`.agents/builder.md`).

## Baseline protocol

The critic captures `origin/main` and the PR head on the same seed, viewport, colour scheme and injected clock (`.agents/data/harness.md`, Captures); `loop/rules.sh delta` prints each check as `fixed`, `regressed`, `still-failing` or `still-passing`. A `regressed` check is a Todo of class `ux-checklist`, and a reject if it is in `just check`. A `still-failing` check is queued; it is a defect only once a scenario names it.
