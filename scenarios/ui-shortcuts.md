# UI: scoped shortcuts (U479 to U481) and the ⌘K leader (U2238 to U2241)

Policy: U3787b (`scenarios/ui-mac-expectations.md`) adds menu-bar discoverability without widening App key ownership. U3787a adds ⌘Z for item restoration only outside editors and Terminals; their own undo remains untouched.

The keyboard layer serves the Enter and Intervene stages of `.agents/data/journey.md`: opening Help must not change the user's selected Agent or place in the centre. This specification defines implementation acceptance; it claims no shipped registry.

Scope (policy): `apps/desktop/src/keys/`, `apps/desktop/src/app/`, `apps/desktop/src/App.tsx` and only Help's rules in `apps/desktop/src/styles.css`. Dependency: #1629 supplies `ui/Dialog` and is completed. Its implementation did not install a shortcut library; the registry implementation adds the selected dependency and regenerated lockfile. Other directories' listeners remain their UI lanes' responsibility. No RPC, App seam command or domain model change is required.

## Reuse

Use [tinykeys](https://github.com/jamiebuilds/tinykeys), whose `createKeybindingsHandler` can match chords behind one listener. Its [package metadata](https://registry.npmjs.org/tinykeys) reports 4.0.1 published 2026-09-25. Matching and modifier parsing belong to that library; the App supplies scope eligibility and action callbacks.

[Solid Primitives keyboard](https://github.com/solidjs-community/solid-primitives/tree/main/packages/keyboard) is maintained ([keyboard package change d4abe795](https://github.com/solidjs-community/solid-primitives/commit/d4abe7959041fb9e90e66f67cadd14e190e2c46e), 2026-07-04). It provides reactive key state and shortcut helpers. Do not select it here: reactive key state is unnecessary for event arbitration, and its shortcut helpers do not provide the standalone event-to-binding handler needed behind this registry's one listener. Neither library owns the App's focused regions or open layers; only that arbitration remains App code. Component focus, dismissal and navigation remain Ark UI behaviour under `docs/adr/0009-app-component-layer.md`, not another shortcut engine.

**U479 scope decides which action receives a shortcut.** Given the same chord registered at App scope, in two mounted regions, and in an open layer, when the user presses it, then only the open layer's action runs. After the layer closes, only the region containing keyboard focus runs; after focus leaves both regions, only the App action runs. Repeat with registration order reversed: the results are identical. With nested open layers, only the innermost active layer receives its matching shortcut. With nested regions, only the innermost containing focus is eligible. Tests: `u479_scope_precedence_ignores_registration_order` and `u479_nested_scope_uses_the_innermost_active_owner` in `apps/desktop/src/keys/`.

An open modal layer blocks underlying region and App shortcuts even when it binds no action for that chord. Outside a modal, an unmatched region chord can reach App scope. A handled event invokes at most one action and prevents the chord's default action; an unhandled event retains native behaviour. Tests: `u479_modal_blocks_unbound_shortcuts_and_region_misses_reach_app` and `u479_handled_events_act_once_and_unhandled_events_keep_defaults`.

Registrations follow their Solid owner's lifetime: disposing a region or layer removes its eligibility; switching Projects never calls a previous Project's action; unmounting and remounting the App does not duplicate dispatch. Tests: `u479_disposal_and_project_switch_remove_stale_actions` and `u479_remount_has_one_dispatch`. No new repeat policy, text-field exclusion or Terminal interception replaces the existing shortcut scenarios: the migrated bindings preserve their existing guards, modifier sets and repeat behaviour.

**U480 one table and one owned listener.** Given the mounted App, when a shortcut owned by this scope is pressed, then its action is selected through the registry and its matching chord comes from `keys/chords.ts`. Help reads that same table; every displayed shortcut has a binding, and every Help-eligible binding has a displayed row. Retain component-navigation hints as hints, not registry actions. Tests: extend the existing U54 drift observer in `apps/desktop/src/keys/u54_help.test.tsx`, The App's Models action and its `u480_app_models_shortcut_uses_the_active_project` test are retired by U377: `⌘,` does nothing.

Policy: move all keydown listeners in `keys/`, `app/` and `App.tsx` into one registry module in `keys/`; the registry exposes registration for later UI lanes. Move existing key actions from these files without adding new actions or changing behaviour. Component keyboard behaviour, including tree arrows, menu navigation, dialog Tab trapping and Escape dismissal, comes from Ark UI; do not encode a new component state machine in the registry. Existing tree behaviour in `keys/Keys.tsx` remains passing during adoption; its ownership migration must coordinate with the Rail lane rather than editing `rail/` here. No claim that this scope removes all 23 listeners.

Policy observer: inspect all production keydown subscriptions in the owned files, including alternate quote styles and helper subscriptions. The command below must list only the registry module; no parallel listener or per-binding library subscription hides elsewhere in this scope. Exit 0 alone does not establish this policy: inspect the returned paths.

```sh
rg -n 'addEventListener\("keydown"' apps/desktop/src/keys apps/desktop/src/app apps/desktop/src/App.tsx --glob '!*test*'
```

**U481 closing Help restores focus without acting beneath it.** Given an open Project and keyboard focus on the centre pane's non-editable frame, when `?` opens Help and Escape closes it, then Help is gone, the exact previously focused element has focus, the selection is unchanged, no underlying Escape action ran and no Terminal input was sent. Test: `u481_help_escape_restores_focus_without_underlying_action`, in a headless real-browser harness test under `apps/desktop/src/keys/`, using the actual App and Dialog, not a mocked focus trap. This extends U54's allowed pane-frame origin; it does not allow `?` in text fields, Terminals, Drawers or the Switcher.

Given open Help, Tab and Shift+Tab keep focus inside it; `?` again and outside click each close it and restore the origin. If the origin is removed while Help is open, closing never focuses a disconnected node or throws. Tests: `u481_help_dialog_traps_and_returns_focus` and `u481_help_close_after_origin_removal_is_safe`. Policy: Help uses `ui/Dialog`; delete its hand-written focus trap, capture-order comment and dismissal listeners. Ark owns Escape, Tab and outside dismissal; the registry owns opening Help, the Help-specific `?` toggle and suppression of underlying shortcuts. Preserve U54's opening guards, content and daemon-exited availability.

Rendered observer: at 1512×950, run the Help interaction on `tree-40` and inject a rejected call through the harness. Use light/dark and increased contrast for the Help interaction behavior observer; captures and critic Check readings use dark only (U3725). The design critic judges the rendered Checks D2, D5 and D6, defined by `docs/design-system.md`, heading `The checks` (AGENTS.md rule 10); `u481_help_escape_restores_focus_without_underlying_action` keeps the behaviour. Moves D2, D5 and D6; only Tokens are used and no Check passing on main regresses. These browser results establish webview behaviour; the user reports what they meet on the Mac.

## Regression and validation

Policy: all shortcut clauses in `scenarios/ui-keyboard.md` remain passing, including U140's focused-row F2 and U358's modifiers, repeat, focus and Help row. U141 remains retired by U190. Preserve U31/U32/U41 and U54 in `scenarios/ui.md`, including Terminal forwarding and every existing Help drift assertion. Run existing tests alongside the new observers; editing a test file also requires the rule 10 duplicate-proof audit.

Implementation records command results on its Issue, one row per failing command with its actual exit code and repair; a failed observer is never reported as a pass. Required commands: `just check origin/main`, the named browser observers and the owned-listener search. The specification PR runs `loop/rules.sh vocab`, `loop/rules.sh scenario-ids` and `just check origin/main`; named new tests above are implementation obligations, not asserted to exist on main.

**The Ask bar shortcut.** `⌘/` opens the Ask bar over whatever the Rail selects and closes it again (`scenarios/ask-bar.md`); it clashes with no chord in `apps/desktop/src/keys/chords.ts`, and Help lists it as `Ask bar`. Test: `u504_the_shortcut_opens_it_over_a_door_and_escape_returns`.

## The ⌘K leader (U2238 to U2241)

Direction: Issue #2335, extended by the Ask bar and U2742c/d. roundup keeps ⌘N, ⇧⌘N, ⌘T, ⌘K and ⌘/ at App scope, plus the scoped ⌘B switch; every other key reaches the focused Terminal. Bar: Ghostty's default keybinds, Claude Code's keybinding list and Apple's standard Mac shortcuts; ⌘N and ⌘T are the new window and new tab of Mac terminals, so they take nothing from a Terminal. Terms: Agent, Terminal, Bubble, Shelf, Project, Rail (`GLOSSARY.md`). The leader serves the Intervene stage of `.agents/data/journey.md`: the user moves between Agents without roundup taking keys from the Agent's own Terminal. This specification defines implementation acceptance; it claims no shipped leader. The implementation Issue is blocked by this one.

Scope (policy): the same directories as U479, plus `apps/desktop/src/copy.ts` and only the leader panel's rules in `apps/desktop/src/styles.css`. No RPC, App seam command or domain model change is required. The leader builds on U479's registry and U480's table: `keys/chords.ts` holds the leader and its keys.

**U2238 the direct keys.** Given an open Project, when the user presses `⌘N`, `⇧⌘N` or `⌘T`, then each does what U32 and U33 say, wherever focus is (a Terminal included), and the webview handles it so a Terminal never sees it. U32's chord rules hold unchanged: Caps Lock neutral, key repeat acts once, other modifier sets do nothing. Tests: the existing `u32_` and `u165_` observers stay passing; no new test repeats them.

**U2239 the leader takes the next key.** Given an open Project and no modal layer, when the user presses `⌘K` (the same chord rules as U32), then roundup waits for one key and nothing else happens. The next key, matched by key position as U32 says and with Caps Lock neutral, runs one action and ends the wait:

| Key | Action |
|---|---|
| `J` | jump to what needs you (U30) |
| `⇧J` | jump to what needs you in another Project (U186) |
| `K` | open the switcher (U51) |
| `M` | minimise or restore the focused Bubble (U165) |
| `W` | close the focused Bubble (U165) |
| `S` | show or hide the Shelf (U370) |

The action is the one the removed chord ran, with its guards (an `M` or `W` with no focused Bubble does nothing, as `⌘.` and `⇧⌘⌫` did). The leader has no timeout. `Escape`, or any key that is not in the table (a letter, digit, `⌘`, `⌃` or `⌥` combination, `Enter`, `Tab`), ends the wait, runs nothing and reaches no Terminal, no field and no Rail; a lone modifier press (`⇧` before `J`) neither ends the wait nor counts as the key. A pointer press anywhere, `⌘K` again, a Project switch, or a modal layer opening ends it too. The key that ends the wait is handled by the webview with `preventDefault`, and its key repeat does nothing. While a modal layer such as Help or the switcher is open, `⌘K` arms nothing (U479). Tests: `u2239_each_leader_key_runs_its_action_once`, `u2239_the_leader_waits_without_a_timeout` (a fake clock moves an hour; the next `J` still jumps), `u2239_escape_and_an_unbound_key_end_the_wait_and_reach_nothing` (including `⌘J`, `⌥J`, `⌃J` and `Enter` with a Terminal focused: no `terminal.write`, no action, then a plain `j` reaches the Terminal), `u2239_a_modifier_alone_keeps_the_wait`, `u2239_pointer_press_project_switch_and_modal_end_the_wait`, in `apps/desktop/src/keys/`.

**U2240 the leader shows its keys.** Given the leader is waiting, then at once (the same frame as `⌘K`) a panel lists the six keys of U2239 as `<key>  <what it does>`, read from the same table as Help (U480) so that no key or label lives outside it. The panel takes no focus and disappears in the frame the wait ends; any animation is under 120 ms and none under `prefers-reduced-motion`. Behavioral observers retain light/dark and increased contrast; captures and critic Check readings use dark only (U3725); only Tokens. Moves D2, D5 and D6, defined by `docs/design-system.md`, heading `The checks`. Tests: `u2240_the_panel_lists_the_table_and_follows_the_wait` and `u2240_the_panel_keeps_focus_and_the_failure` (a headless real-browser test under `apps/desktop/src/keys/`; the design critic judges the Checks, AGENTS.md rule 10; `loop/rules.sh tokens` passes). These browser results establish webview behaviour; the user reports what they meet on the Mac.

**U2241 nothing else is taken.** U3799 adds `⌘,` for App Settings at App scope; U3737/U3804 add the one configured Todo quick-add chord (initially `⇧⌘T`). These are the only additional exceptions below. U2742c/d adds only scoped `meta+KeyB` to the allowlist: main Agent emulator, Bubble or terminal tab, with no modal/editor ownership. Keep `⌘/` (Ask bar) and every other existing exception. Update the modifier sweep and Help drift observers for that scope, without allowing `⇧⌘B` or cycling. Given an open Project with a Terminal focused (a Bubble or an Agent's pane), when the user presses any `⌃` combination, any `⌥` combination or any `⌘` combination other than `⌘N`, `⇧⌘N`, `⌘T`, `⌘K`, `⌘/`, `⌘,`, the configured U3804 quick-add chord and the scoped U2742c `⌘B`, over letters, digits and punctuation, with and without `⇧`, then no roundup listener calls `preventDefault`, no action runs and the Terminal's key handler receives the key. The Terminal's own `⌘C`, `⌘V`, `⌘=`, `⌘-` and `⌘0` act in the Terminal screen as before. Keys that act only while focus is inside roundup's own panels are unchanged: a Rail row's `⌘⌫` (U358), the switcher's `Escape`, list arrows and `?` for Help (U54). Help lists exactly these kept keys: `⌘N`, `⇧⌘N`, `⌘T`, `⌘/`, `⌘,`, U3804’s configured quick-add chord, U2742c's scoped `⌘B`, `⌘K` with its leader keys, and the panel-scoped rows it listed before for `⌘⌫`. Tests: `u2241_no_other_chord_is_taken_at_app_scope` (every `CHORDS` row without `inRail` has `presses` equal to `meta+KeyN`, `meta+shift+KeyN`, `meta+KeyT`, `meta+KeyK`, `meta+Slash`, `meta+Comma`, the configured U3804 quick-add press or scoped `meta+KeyB`, and a sweep of the modifier matrix dispatched at a focused Terminal sees none handled), `u54_question_mark_in_the_rail_opens_help_listing_each_chord_and_esc_returns_focus` and `u480_u54_help_stays_in_step_with_the_chords_bound_in_the_webview` (the rendered-list and binding drift observers). Apart from the configured U3804 quick-add chord, roundup takes no `⌃` or `⌥` key and adds no keyboard navigation between Agents.

### Retired by this change

Under rule 10 the implementation deletes these clauses and their tests; it migrates none. Each line names the scenario clause that retires and what replaces its proof; the buttons they duplicate stay.

| Clause | Scenario | Retires |
|---|---|---|
| `⌥1` to `⌥9`, `⌥0` take over and end the Takeover | U107; U375 and U531 where they name the keys | the keys; #3563 later retired the Takeover and every control for it (U107, U375, U530 to U532) |
| `⌘B`, `⇧⌘B` next and previous Bubble (U2742c permits only the new scoped two-way ⌘B switch) | U165; `scenarios/ui-terminal-tabs.md` Keys and Pull; `scenarios/ui-read-output.md`, `scenarios/ui-bubble-pull.md` where they name the keys | the keys; chips stay clickable |
| `⌘1`, `⌘2` focus the Rail and the pane | U41 | the keys |
| `⇧⌘]`, `⇧⌘[` change Project | U186; `scenarios/project-tabs.md` Keys | the keys; the tabs stay clickable |
| `⌘,` Models | U377, U480's Models test | the key and the Models button; U377 retires the sheet |
| `⌘J`, `⇧⌘J` | U30, U45, U186 | replaced by leader `J`, `⇧J` (U2239) |
| `⌘.`, `⇧⌘⌫` | U165 | replaced by leader `M`, `W` (U2239) |
| `⌘\` | U370 | replaced by leader `S` (U2239) |
| `⌘K` opens the switcher | U51 | replaced by leader `K` (U2239) |
| the Help rows of all the above | U54 | Help lists what U2241 says |

U504's Ask bar shortcut is not in this set; a later scenario adds it as a leader key, never as a direct chord. U54's order list, U32's "same keys with Shift, Ctrl or Alt do nothing" and U358's modifier clauses stay for the keys that remain.

Policy observer: `u2241_no_other_chord_is_taken_at_app_scope` is the one check that fails a chord added at App scope beyond `⌘N`, `⇧⌘N`, `⌘T` and `⌘K`. The specification PR runs `loop/rules.sh vocab`, `loop/rules.sh scenario-ids` and `just check origin/main`; named new tests above are implementation obligations, not asserted to exist on main.
