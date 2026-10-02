# Queue rows from the $anchor rules (architect-b)

Scenarios first, no Builder yet. Ids U105 to U110 are in `scenarios/ui-attention.md`, B24 to B28 in `scenarios/messages.md`; they sit in U100–U129 and B1–B29, already reserved.

| ids | what | owned files | depends on |
|---|---|---|---|
| U105, U106, U110 | typing dims the periphery; folded lines; select-to-quote | `apps/desktop/src/**` (Thread input and feed) | the Door scenarios (not yet written), U51 |
| U107 | jump with `⌥1`–`⌥9`, `⌥0`, `@` | `apps/desktop/src/rail/**` | B6's `takeover.begin` Builder PR, U51 |
| U109 | Attachment and hover preview | `apps/desktop/src/**` (Thread input) | the Door scenarios |
| B24 | the Meta-agent's Brief carries the gates | `crates/rupd` (Brief text) | E1, E3 on main |
| B26 | one Touch and a reason per status change | `crates/messages/**` | the Messages store Builder PR |
| B28 | a failed child reported with its last Touch | `crates/messages/**`, `crates/rupd/src/**` | B21's Builder PR |
| U108, B25, B27 | spikes: shell Drawer, spec gate, Evaluators | none | spike results before any scenario for a Builder |
