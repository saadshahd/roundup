# J1 journey audit

Main: `84599c3`. Open PRs: #354 (H18), #352 (B1–B18), #349 (U134, U135), #330 (F7 re-run). Chromium on the same Mac, light theme, `just harness <seed> 5199` (Vite on the fake Daemon), agent-browser, 1280×800 and 700×800. `<stage>-<width>.png` and `.checks.json` (the complete `window.__checks()`) are in this directory. Seeds: `first-run`, `empty-project`, `door-stopped`, `conflict` (Enter), `agents-10` (Understand). Nothing here ran a real Daemon, a real `claude` or the native App.

D3, D4, D5, D8 and D9 pass on every capture. D1 and D10 fail on all ten, D2 on eight (all but `first-run`), D6 and D7 fail only on `door-stopped`. These failures predate this audit; none is introduced or excused here.

| Stage | Observed | Verdict |
|---|---|---|
| Enter | `first-run` offers `choose folder…`; `empty-project` and `door-stopped` carry U143's first action and Door state (its `u143_` tests are on main). Stopped Door fails D6 and D7 at both widths. The native folder chooser and a real Door launch are unobserved. | Unproved: no Daemon-backed run from `first-run` to a Thread. |
| Request | No rendered or Daemon observer asks a Door for one action and reads the resulting Todo and outcome. The only way in is the fake App; real `claude` interaction is F7's open re-run (#330). | Unproved. |
| Understand | `agents-10`: error, needs-you, blocked and working rows differ by glyph and word, the header says `4 need you`, and the centre says `select an agent or a terminal`. The ten-Agent screen fails D1 and D2; no Room is selected in this seed, so selected-Room identification is unobserved. | Unproved: selected Room and routine-progress silence are not observed. |
| Intervene | The seeds show needs-you rows but no Decision Card: U113 is waiting on H18 (#354, in flight), so the user cannot answer anything in the App. | Unproved; owned by H18 then U113. |
| Review | No Room-scoped result view; reopen is covered by the Daemon's A8 and S5 tests, not by a rendered observer. | Unproved. |

## First unproved behavior

Earlier-run Agent or Terminal with no saved output: selecting one leaves a blank pane with no explanation (a Review/Intervene dead end), already specified as **U83** (`scenarios/ui-surfaces.md`, ready, no PR, no Claim). It is the only independent gap with an owner-less ready row; H18/U113 (Intervene) and B1–B18 are owned by open PRs, and F7 by #330. No new implementation row is added. J2 waits on U83.

# J2 journey audit

Main: `4649c5f` (holds U83, H18, U143; open PRs #393 U113, #396 B1–B18, #349 U134/U135, #330 F7 re-run). Same method as J1: `just harness <seed> 5199`, agent-browser, Chromium, light, 1280×800 and 700×800, all twelve seeds-by-width read with `window.__checks()`. New captures: `j2-earlier-run-*` and `j2-agents-10-*` (`.png` and `.checks.json`). The J1 captures of `first-run`, `empty-project`, `door-stopped` and `conflict` were repeated and read the same: `first-run` fails D1, D10; `empty-project` and `conflict` fail D1, D2, D10; `door-stopped` additionally fails D6 and D7 (`textarea.xterm-helper-textarea`: no hover state, no pressed state) at both widths. Nothing here ran a real Daemon, a real `claude` or the native App.

| Stage | Observed | Verdict |
|---|---|---|
| Enter | Unchanged since J1. The webview reaches a Daemon only through Tauri commands, so no browser run can go from `first-run` to a Thread on a real Daemon. | Unproved: needs U144. |
| Request | Daemon half: `b24_` and `d3_` in `crates/rup/tests/e2e.rs` drive a Door over MCP against the fake `claude`; no rendered run reads the Todo in the Shelf. Real `claude` is F7's re-run (#330). | Unproved: needs U144 for the rendered half. |
| Understand | `agents-10` unchanged: glyph and word separate error, needs-you, blocked, working; header `4 need you`; no Room selected, so selected-Room identification stays unobserved. | Unproved. |
| Intervene | No Decision Card renders on main; U113 (#393) is in flight. | Unproved; owned by U113. |
| Review | U83 now renders: `earlier-run` selected shows `no output kept from an earlier run` at both widths, so that dead end is closed. No result view of a Room and no rendered reopen. | Unproved: needs U144 for reopen; Room-scoped results have no contract. |

## Gap selected

Enter, Request and Review share one missing observer: the App cannot be rendered on a real Daemon outside the native App. Specified as **U144** (`scenarios/ui-first-journey.md`), After U143 (done), so ready. Intervene stays with U113 and the Meta-agent spike with #330; neither is duplicated. **J3** waits on U144 and repeats this audit through `just harness-real`.

Loop note, not changed here: `loop/rules.sh ready` marks a policy row done only by a `j<n>_` test, so J1 and J2 print `ready` after merging.

# J3 journey audit

Main: `caf5589` (holds U144; open PRs #461 and #468 are loop machinery only). Method: `rupd <tmp project> --attached` with the fake `claude` and `RUPD_SOCKET`, then `just harness-real <project> 5199`, agent-browser (Chromium, light) at 1280×800 and 700×800, `window.__checks()` read after each step. Captures: `j3-enter-room-1280`, `j3-request-1280.png`, `j3-daemon-gone-1280.png`, `j3-review-reopen-*` (`.png`, with `.checks.json` where a Check ran). Nothing here ran a real `claude` or the native App.

| Stage | Observed | Verdict |
|---|---|---|
| Enter | From the empty Project, `start a Room` created a Room and started its Door on the real Daemon; the Rail row and a Terminal came from it. D1 and D10 fail on the empty screen; with the Room, D5 (`span.live-label`, 4.18:1), D6 and D7 (`textarea.xterm-helper-textarea`) fail too at 1280. The Thread input sits flush with the window's bottom edge (y 776 to 800), also with no Room (U145, #440). The native chooser, the keyboard path and failure states were not re-run. | Unproved as a stage; the pointer path reaches a Thread on a real Daemon. |
| Request | `ship j3` typed in the Thread reached the Daemon and showed as `you → room note …`. After 15 s the Room still read `starting`, the Thread showed no delivery state, and `todos` read `no todos yet`: the fake `claude` is scripted only by its start-up environment and never reads typed text. | Unproved: no Door action, Todo or outcome is observable. |
| Understand | Not driven: the real Daemon holds one Room. `agents-10` is unchanged from J2. | Unproved. |
| Intervene | Not driven; U113 owns the Decision Card. | Unproved. |
| Review | Killing `rupd` showed `daemon exited by signal` with `reopen`. After a restart and reload the Room and the Thread's Message were found again; the Room read `Door stopped` with `start Door`. D6 and D7 pass at both widths (D1, D5, D10 fail). No Todo or result existed to find. | Reopen of a Room and its Thread proved on the real Daemon; result inspection unproved for want of a Request outcome. |

## Gap selected

The first unproved behavior is Request: nothing makes a Door act on a Thread message. Specified as **U146** (`scenarios/ui-first-journey.md`): `FAKE_CLAUDE_ON_PROMPT` in the fake `claude`, the Message's `delivered` status in the Thread, and the Todo in the Shelf, observed by `u146_` tests on the real Daemon. U145 (#440) and U113 keep their owners. **J4** repeats this audit once U146 is on main.

# J4 journey audit

Main: `b2a75608` (holds U146; open PR #515 is U150 only). Method: `rupd <tmp project>` (not attached) with `RUPD_SOCKET`, `ROUNDUP_CLAUDE_BIN` the fake `claude` and `FAKE_CLAUDE_ON_PROMPT='{"name":"todo_create","arguments":{"title":"$prompt"}}'`, then `just harness-real`'s Vite on port 5199, agent-browser (Chromium, light), `window.__checks()` read after each step. `u146_a_message_to_a_started_door_is_delivered_and_its_todo_shows` passes on this main (`pnpm exec vitest run src/testing/u146_door_answers.test.tsx`). Captures: `j4-request-1280.png` and `.checks.json`, `j4-review-todo-1280.png`, `j4-review-reopen-1280` and `j4-review-reopen-700` (`.png` and `.checks.json`). The Request step was captured at 1280 only. Nothing here ran a real `claude` or the native App.

| Stage | Observed | Verdict |
|---|---|---|
| Enter | `start a Room` created the Room and started its Door; the Rail row read `idle`. Not re-run by keyboard or on the failure states. | Unproved as a stage; the pointer path holds as in J3. |
| Request | `ship j4` typed in the Thread and sent: the Thread read `you → room note ship j4 delivered`, the Shelf listed `#1 ship j4`, the Room read `idle`, with no reload. The Terminal shows the fake's raw `^[[200~[from you, note] ship j4^[[201~` (the fake's tty echo, not a product claim). D3, D4, D8, D9 pass; D1, D2, D5, D6, D7, D10 fail. | Proved on the real Daemon with the fake `claude`; a real `claude` is F7's. The result is a Todo only: no Door summary exists (no method, `ui-first-journey.md` Capability paragraph). |
| Understand | Not driven: the real Daemon holds one Room. | Unproved. |
| Intervene | Not driven; U113 (#380) owns the Decision Card. | Unproved. |
| Review | The Todo opens in its Drawer (`ship j4`, `no body`). After `rupd` was killed and restarted and the page reloaded, the Rail read `done`, `terminal gone` with `start Door`, the centre `Door stopped`, the Thread still showed the Message `delivered` and the Shelf the Todo. D6, D7 pass; D1, D2, D5, D10 fail, at 1280 and 700. Nothing ties the Todo to the Room: `todo.create` has no Home (`Todo` has no Room field, T12 is specified but unbuilt), and the Shelf lists the whole Project. | Reopen proved. Finding the Door's work from its Room is unproved: no Home, no Room view. |

## Gap selected

The first unproved behavior is Review: the Door's work cannot be found from its Room. T12 and T13 (Home, `todo.move`) and U151, U157 and U158 (Home line, move, follow the Room) are specified with no implementation Issue; this audit files those two (#516, #517). T14 (#518: an Agent's Todo takes its Room as Home) and U159 (#519: the Shelf's `this room`) are new. Each Issue is blocked by the one before it. **J5** (#520) repeats this audit once they are on main. The D1, D2, D5, D10 failures predate this audit and keep their owners (#382, #385, #386).

# J5 journey audit

Main: `0a507eea` (holds T12, T13, T14, U157, U158, U159; open PRs are loop machinery only). Method: `rupd <tmp project>` (not attached) with `RUPD_SOCKET`, `ROUNDUP_CLAUDE_BIN` the fake `claude` and `FAKE_CLAUDE_ON_PROMPT='{"name":"todo_create","arguments":{"title":"$prompt"}}'`, then `just harness-real`'s Vite on port 5199, agent-browser (headless Chromium, light), `window.__checks()` read after each step. Captures: `j5-request-1280.png`; `j5-review-this-room-1280`, `j5-review-todo-1280`, `j5-review-reopen-1280`, `j5-review-reopen-700`, `j5-two-rooms-1280` (`.png` and `.checks.json`); `j5-door-reply-1280` and `-700` (a second `rupd` whose fake answers with `message_send {to: "you"}`). Nothing here ran a real `claude` or the native App.

| Stage | Observed | Verdict |
|---|---|---|
| Enter | `start a Room` created a Room and started its Door; a second and third Room came from `+ room`. A stopped Room's `start Door` ended as `retry Door` after a Daemon restart (#351 owns the stopped-Room start). Keyboard and failure states not re-run. | Unproved as a stage; the pointer path holds. |
| Request | `ship j5` typed in the Thread: the Thread read `you → room note ship j5 delivered`, the Shelf listed `#1 ship j5`, the Room read `idle`. With the fake calling `message_send {to: "you"}`, `room → you note done j5` reached the Thread. A fake Door's reply and Todo are proved; a real `claude` stays F7's. | Proved on the real Daemon with the fake `claude`. |
| Understand | Three Rooms exist on the real Daemon. All three read `room` on the Rail, and the Thread of the newest Room listed Messages sent to the other two (`j5-door-reply-1280.png`). Ten Agents and routine-progress silence were not driven. | Unproved: the Thread is not Room-scoped (U160). |
| Intervene | Not driven; U113 (#380) owns the Decision Card. | Unproved; owned by U113. |
| Review | The Todo the Door made showed under `this room` and its Drawer read `in room` (T14, U158); moving it to `project root` emptied `this room` (`no todos in this room`) and moving it back restored it; a second Room's `this room` read `no todos in this room`. After `rupd` was restarted and the page reloaded the Rail read `done`, `terminal gone`, `start Door`, the Thread kept the Message and the Todo was found again (the choice is back to `all`, as U159 says). D3, D4, D8, D9 pass; D1, D2, D5, D6, D7, D10 fail at 1280 and 700, in every capture. With the Drawer open the row title is missing and the move control overlaps the `pads` header at 1280 (`j5-review-todo-1280.png`); the Todos panel is #398's. | Proved: the Door's work is found from its Room and again after reopen. The Door's summary when work settles (P1) has no method and is not claimed. |

## Gap selected

The first unproved behavior is Understand: with more than one Room the Thread mixes every Room's Messages, so the Door's outcome is not read from its Room. Specified as **U160** (`scenarios/ui-first-journey.md`) with one implementation Issue; no prerequisite is missing. Intervene stays with U113 (#380) and the stopped-Room start with #351. **J6** repeats the audit once U160 and U113 are on main. The D1, D2, D5, D6, D7, D10 failures predate this audit and keep their owners (#382, #385, #386).
