# Design system: quiet native

How the App looks. `docs/wireframes.md` says what each screen holds and where; this file says how it is drawn. Where the two disagree on look, this file wins: it replaces the wireframes' "ink is the only colour" and "no borders" rules (the Glyph set, the visibility rule and the layout stay).

The direction: a Mac app that stays out of the way. System sans for the chrome, mono only where text is code. Soft surfaces, small coloured Glyphs, nothing that moves unless the user did something. Light and dark.

Every rule below is a check with an id. A check passes or fails; it has no score. The same checks run twice: as `vitest` tests in `just check` (the gate), and by the Design critic on the real rendering (advisory). One module measures them for both (U137).

## Five rules, and where each comes from

| # | Rule | Source | Checks |
|---|---|---|---|
| 1 | Every look value is a Token; no other value is drawn. | Frost: inventory the parts, then fix them in one place | D1, D2 |
| 2 | Hue is rare and means one thing: a Kind, or the one accent. | Tufte: colour that carries status stays scarce | D3, D4 |
| 3 | Anything clickable looks clickable and answers at once. | Norman: signifiers and feedback | D6, D7 |
| 4 | A check passes or fails on measured values, on screens the Builder did not tune for. | Goodhart: a score the author can see becomes the target | D1 to D10, the held-out list |
| 5 | A visual change is judged against the build before it, not against taste. | Deming: study the change, not the noise | the baseline protocol |

## Tokens

All Tokens live in one file, `apps/desktop/src/tokens.css`. No other stylesheet, and no inline style, holds a colour, font size, radius, shadow or duration literal; it reads `var(--…)`. Spacing uses the space Tokens, and `0`.

Starting values. The thresholds in the checks are fixed; a value that fails a threshold is changed, never the threshold.

| Token | Light | Dark | Use |
|---|---|---|---|
| `--ground` | `#ffffff` | `#1c1c1e` | the pane and the Drawer |
| `--sunken` | `#f5f5f7` | `#141416` | the Rail and the Shelf |
| `--hover` | `rgba(0,0,0,.04)` | `rgba(255,255,255,.06)` | a row or button under the pointer |
| `--selected` | `rgba(0,0,0,.07)` | `rgba(255,255,255,.10)` | the selected row |
| `--text` | `#1d1d1f` | `#f5f5f7` | body text |
| `--grey` | `#6e6e73` | `#a1a1a6` | secondary text, Glyph tone of `blocked`, `idle`, `done` |
| `--accent` | `#0a60d8` | `#4d9bff` | focus ring, Glyph tone of `working`, the primary action |
| `--amber` | `#9a5b00` | `#f0a030` | `needs-you` |
| `--red` | `#c4262e` | `#ff6b6b` | `error` |
| `--hairline` | `rgba(0,0,0,.10)` | `rgba(255,255,255,.12)` | the one permitted border |
| `--shadow-drawer` | `0 8px 32px rgba(0,0,0,.14)` | `0 8px 32px rgba(0,0,0,.5)` | the Drawer only |
| `--radius-row` / `--radius-control` / `--radius-drawer` | `6px` / `6px` / `12px` | same | selection band, buttons, Drawer |

Type: `--font-ui` is `-apple-system, system-ui, sans-serif`; `--font-mono` is `ui-monospace, "SF Mono", Menlo, monospace`. The steps are `--text-caption` 11, `--text-small` 12, `--text-body` 13, `--text-title` 15 px. Weight is 400, 500 or 600. Tracking is `0` at body and below, `-0.01em` at title (apple-design: tracking follows size).

Space: `--space-1` to `--space-6` are 4, 8, 12, 16, 24, 32 px. A Rail row is 28 px high.

Mono is for the Terminal, paths, ids and code. Every other word is `--font-ui`.

## Colour

Kind has two carriers, a Glyph shape and a tone. The shape is enough on its own: colour never carries a Kind alone. Ink (bold plus hue) stays what `CONTEXT.md` says: only `needs-you` and `error`.

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
| rest | its resting surface; a button also has a label in `--text` or `--grey` and a hit height of at least 24 px | always |
| hover | `--hover` surface, cursor `pointer` | pointer over it |
| pressed | one step darker than hover, with no transition | from `pointerdown`, before `pointerup` |
| focus | 2 px `--accent` outline, offset 1 px | `:focus-visible` |
| disabled | `--grey` text, cursor `default`, `aria-disabled` | when it cannot act |

A word such as `stop`, `close` or `promote` is a button, not text: it has the hover surface and a 24 px hit height even though it rests as plain `--grey`.

## Surfaces and depth

The Rail and the Shelf sit on `--sunken`; the pane and the Drawer on `--ground`. Regions are told apart by that change of ground and by space, not by a drawn line. The one border allowed is `1px solid var(--hairline)`, where a region meets one of the same ground. Only the Drawer casts a shadow. A translucent surface (`backdrop-filter`) is never placed over another (apple-design: never stack light materials), and the terminal never sits under one (U5).

Three user settings change the look. Each has a rule in `tokens.css`:

- `prefers-color-scheme: dark` uses the dark column.
- `prefers-contrast: more` makes every `--hairline` 40% opaque and every `--grey` text `--text`.
- `prefers-reduced-transparency: reduce` removes every `backdrop-filter`.

Motion is `docs/motion.md`, unchanged.

## The checks

Each check is measured from computed style and the DOM of the running App, never from pixels, on every visible element of the screen, in light and in dark. A check that cannot measure fails; it never passes by default.

| Id | Passes when | Scenario |
|---|---|---|
| D1 | every computed colour, background, border-color, shadow, radius and duration is the value of a Token | U130 |
| D2 | every font size is a type step; every margin, padding and gap is `0` or a space step | U131 |
| D3 | every colour whose saturation exceeds 15% is `--red`, `--amber` or `--accent`; with no `error` or `needs-you` row on screen, no red or amber is drawn | U132 |
| D4 | every Rail, Shelf and Drawer row shows one of the six Glyphs for its Kind; no Kind is drawn by colour only | U132 |
| D5 | text has at least 4.5:1 against its own ground, and a Glyph tone and a focus ring 3:1 (`done` is exempt as U4 says), in light and in dark | U4, U135 |
| D6 | every click target shows its rest, hover and focus states from the table above, and its hit area is at least 24 px high | U133 |
| D7 | every click target's pressed style differs from hover, appears at `pointerdown` and has a transition of `0s` | U133 |
| D8 | no border is wider than 1 px or any colour but `--hairline`; only the Drawer has a shadow; no `backdrop-filter` element contains another | U134 |
| D9 | each duration is within 25% of `docs/motion.md`, and under `prefers-reduced-motion` all but the instant changes are 0 | U5, U135 |
| D10 | D1 to D9 hold in both colour schemes, in `prefers-contrast: more`, and at 700 by 800 | U135 |

## Held-out screens

A check is public. The screens it runs on are not all the Builder's. The Builder drives the scenario's own seeds. The critic also runs the screens listed in `.agents/design-critic.md`, which a Builder prompt never carries. The Architect changes at least one entry in that list after every ten merged UI PRs. This stops a PR tuning one screen to pass; it does not hide a rule.

## Ideas

The critic's report ends with an `ideas` list of at most three things that look wrong and no check covers. An idea is never a Todo and never a gate. The Architect reads the ideas each batch. An idea that comes back three times becomes a check (`docs/development-loop.md`, "Loop on the loop").

## Baseline protocol

Plan, do, study, act, for each UI PR.

1. **Plan.** The PR names the checks it moves (`Moves: D3, D6`) and the scenarios.
2. **Do.** The Builder writes the failing test first. Each check is a `vitest` test over one module (U137), so `just check` is the gate.
3. **Study.** The critic builds `origin/main` and the PR head, and for each seed and step saves `<step>.png` and `<step>.checks.json` under `artifacts/ux/<id>/base/` and `artifacts/ux/<id>/head/`: each check, `pass` or `fail`, and the measured value and selector behind it. The two builds share the seed, viewport, colour scheme and the injected clock. `loop/rules.sh delta` compares the two sets of `checks.json` files, never the pixels, and prints each check as `fixed`, `regressed`, `still-failing` or `still-passing`; the critic pastes its lines.
4. **Act.** A `regressed` check is a Todo of class `ux-checklist` and a reject if it is in `just check`. A `still-failing` check is queued; it is a defect only once a scenario names it.

Linux and macOS captures are never compared with each other (`docs/boxd.md`).
