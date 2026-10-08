# UI: the first Room (U143)

Module: `apps/desktop` (the webview). Ids: U143. Found by walking the first journey in the U26 harness: an open Project with no row says only `select an agent or a terminal` and offers no first action in the centre; a Room whose Door is not running shows a bare `start Door`, so a Door that never launched, one that stopped, one that is starting and one that failed to start look the same.

**U143 the first Room says where it stands.** Narrows U38 and U9; changes no contract, App seam command or token. Given an open Project, then:

- **Empty.** Only when `rail.tree` has no row, the centre replaces U38's `select an agent or a terminal` with a light `no Room yet` and one button, `start a Room`. The button runs the same path as the Rail's `+ room` (U9): one `rail.createRoom {name: "room", parent: null}`, then select the returned Room once `rail.tree` has its row, then one `rail.startDoor {id}`. It is a native button, so Tab reaches it and Enter or Space activates it. While creation is pending the line reads `creating Room…`, the button is disabled, and a second activation of it or of `+ room` calls nothing (U9). A failed creation shows its message as an alert, selects nothing, starts no Door, and leaves the button ready to try again. A failed Door launch afterwards retries that same Room through U9's `retry Door`; it never creates another. With any row present and none selected, U38's text and no button show. The Rail's `no agents yet` / `⌘N starts one`, `+ agent` and `+ terminal` are unchanged, so direct Agent access stays.
- **Door states.** When the selected row is a Room whose Door has no running program, the centre shows one light state line above U9's `start Door` / `retry Door`, in a fixed block that does not resize the pane. The line is one of:
  - `starting Door…` while a `rail.startDoor` call is pending; the button is disabled, so a repeat activation calls nothing (U9);
  - `Door did not start` after a failed start (U9), beside the `✕ <message>` line and `retry Door`;
  - `Door not started` when the Room has no Attempt (A20) and no Terminal;
  - `Door stopped` when it has an Attempt and no running program, followed by `, exited <code>` or `, exited by signal` when the ending is known (U7's wording). `stopped` means launched before and not running now, however it ended, so a Room whose first launch failed reads `stopped` after a reload.
- **Running.** A Room with a running Door shows its Terminal and no state line. The words never mention a Thread or coordination: today the Thread is the Door's Terminal (`GLOSSARY.md`).
- **Direct access.** With a stopped Door selected, `+ agent` and `+ terminal` still spawn inside that Room (U9).

Tests are named `u143_…`: `apps/desktop/src/rail/doorState.test.ts` (the four states and their words) and `apps/desktop/src/terminal/u143_door_states.test.tsx` (the rendered App against the fake App seam, including keyboard focus, pending and repeat activation). Observer: the U26 harness at 1280 by 800 and 700 by 800, `?seed=agents-10` with its rows removed through `rail.remove` (an already open, empty Project), then `start a Room`, a failed start (`__fake.failNext`), the Rail menu's `stop`, and a `terminal.exited` with a code. `window.__checks()` must not regress against `main`: the full before/after readings and screenshots are in `artifacts/ux/U143/`. D1, D2, D7, D8 and D10 already fail on both observed screens; D6 additionally fails with a stopped Door. D3, D4, D5 and D9 stay passing. This scenario weakens no Check. Percy also captures the `empty-project` and `door-stopped` seeds, with the stopped Room selected before rendering. The native folder chooser is not covered by this browser observer. The fake answers at once, so `creating Room…` and `starting Door…` are observed in Vitest with a held promise, not in a screenshot.

Capability against UI. The Thread's input, Messages to a Door and Routes have contracts (`message.send`, `message.list`, `route.set`, `takeover.begin`, `decision.list`; `contracts/generated/methods.ts`) but no desktop UI calls them; U105 to U110 (`ui-attention.md`) already own that UI and wait on the Door scenarios. Until it lands, a Room's conversation is its Door's Terminal and this slice claims nothing more. A summary of settled work (P1's observer) has no method; Todos have the Project-wide `todo.*` methods and a Shelf UI; their contract has no Room field. Room-scoped Todos need a contract decision before a Builder can implement them.

## Work

Rows a Builder can take; `loop/rules.sh ready` prints each one's state.

| Ids | Item | Owns | Keeps green | After |
|---|---|---|---|---|
| U143 | actionable first Room and Door lifecycle states (Moves: D6, D7) | `apps/desktop/src/terminal/**`, `apps/desktop/src/rail/**`, `apps/desktop/src/state/rail.ts` | U9, U38, D1–D10 | U9 U38 |
