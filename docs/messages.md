# Messages between Actors

Design for `scenarios/messages.md` (B1 to B40) Serves `PRINCIPLES.md` P1, P3 and P4 (the ids in `PRINCIPLES.md`, not `pads.md`'s). Terms are in `GLOSSARY.md`; the one new word is "digest" (the type `Digest`, `agent.digest`, the tool `agent_digest`), the user's choice, defined in `GLOSSARY.md`.

## What exists and what does not

Exists: the `Delivery` type and the `bus.route` Hook type (`contracts/hooks.ts`), the "Message delivery" paragraph of `docs/architecture.md`, Routes named in ADR 0004, the Inbox wireframe, and `type_prompt` in `crates/agents`, the only path that types into an Agent's Terminal. Missing: a Message type, a store, RPC methods, an MCP tool, and any delivery logic.

## Status of a Message

| Status | Meaning | Leaves it by |
|---|---|---|
| `pending` | to be typed when the current Attempt is delivery-ready (`idle` or B37's live `error`) | typed (`delivered`), or the receiver exits (`dropped`) |
| `held` (`ask-first`, `takeover`, `escalated` or `inbox`) | waits for the user, for a Takeover to end, for unsent input to be submitted or cleared, or for the Agent's Claude inbox to pass its check | `message.deliver`, `message.drop`, the end of a Takeover or the submit or clear of unsent input (only `takeover`), the user's answer (only `escalated`), a passing check at the Agent's next `idle` (only `inbox`; `message.deliver` of it needs that check too, else `INBOX_UNAVAILABLE`) |
| `delivered` | typed into the Terminal, or stored for the user | final |
| `dropped` | never typed, with a reason | final |

Default Route is `auto`. A Held Message never blocks a later one (B7). Delivery is exactly once (B39, B40): `delivered` is recorded when the Agent's `UserPromptSubmit` Signal confirms the prompt, and an unconfirmed prompt is typed again. `blocked` is unchanged: this series gives Routes no say in Kind (`GLOSSARY.md`, "Who decides Kind").

## B50 to B52: a `note` goes to the Claude inbox socket

The model's `Type(id)` effect for a `note` (a Message of kind `note` from an Actor that is not the user) is not a Terminal write: the adapter sends the note over the receiver's messaging socket (its Claude inbox), once per id, and `delivered` is recorded when Claude Code's transcript records the line's `msg_id`, in place of the `UserPromptSubmit` Signal. When the inbox check at the `idle` is `missing` or `mismatch`, the `Type` becomes a hold for `inbox` instead, outside `step`: `held` is not `pending`, so the note is neither typed nor lost, and the next passing check at an `idle` makes the held notes `pending` in id order. The laws it touches: M1 (a note is written at most once: an id already in flight is skipped, and Claude Code drops a `msg_id` it has taken up), M6 (a held `inbox` note is `held`, never silently lost) and M10 (a `pending` note at an `idle` is sent or held). `step`, `model.bend` and `LAWS.bend` are unchanged.

## B37 lifecycle-to-model mapping

The model's `Idle` event and `World.idle` mean delivery readiness, not a literal
Kind. For the current Attempt, a running Terminal at `idle` or at `error` after
`StopFailure` is delivery-ready. Kind `error` alone is not evidence of ending.
The Attempt identity and authoritative ending facts must be checked before
projecting Status into model events; a Status observation cannot revive an ended
Attempt. B9 still applies to `done`, an ended Attempt or receiver removal.

| Current receiver fact or action | Existing model interpretation |
|---|---|
| Live current Attempt becomes `idle` or reports `StopFailure` / `error` | `Idle`: flush pending Messages in B7 order, with B30 batching in Rust; do not drop them or wait for another `idle` Signal. |
| New Message while already delivery-ready | `Send` with the current Route and sender; the ready state permits delivery immediately. |
| Route `ask-first` or `drop` | `Send` retains the ask-first hold or drops respectively; readiness bypasses neither Route. `UserDeliver` / `UserDrop` retain their existing guards. |
| Existing Takeover or unsent input | `TakeoverBegin` / `UserInput` keeps automatic delivery held, including Messages from the user (B32). `TakeoverEnd`, `UserSubmit` or `UserClear` releases only its own holds in B7 order; ask-first holds remain. Explicit `UserDeliver` still wins. B37 grants no new `takeover.begin` eligibility. |
| Current Attempt is not ready, or typing is refused as `Busy` | `Busy`: no Type effect; keep the batch pending, or takeover-held if a Takeover began meanwhile. Rust undoes its pre-typing delivery record on refusal; the model's atomic record-and-Type step represents only accepted typing. A newer readiness revision supplies `Idle` again, including when its handler ran before the Busy completion: Rust re-reads authoritative readiness after rollback and retries only at a strictly newer revision of the same Attempt. An unchanged revision does not retry. |
| Current Attempt actually ends, even with last Kind `error`; Kind `done`; or receiver is removed | `Exit`: drop pending and takeover-held Messages as receiver-gone in id order, retain ask-first holds, refuse later sends and manual delivery. A later `Idle` cannot clear `gone`. |
| Observation belongs to an older Attempt or a stale Status revision | No model event for the current receiver: neither `Idle` nor `Exit`. An older `StopFailure` cannot reopen an exited receiver or close a resumed Attempt. |

A resumed Attempt has a new receiver generation; its predecessor's final Messages
stay final (B36). Bus-lag reconciliation and fresh send/release scheduling use the
same current-Attempt facts, never the last Kind alone. A35's explicit reservation
projections below still apply after this lifecycle classification.

M1 retains the conditional record/at-most-once typing boundary; M4 retains queue
ranks and hold-release order; M6 retains every pending or held record on live
failure and Busy refusal; M7 applies `Exit` to actual ending, not live failure;
M10's fair `Idle` progress includes live `error`. Fairness still requires eventual
accepted delivery: an indefinitely Busy receiver cannot promise progress.
`LAWS.bend`, predicates and transitions are unchanged. Proofs cover the model,
not Rust lifecycle classification, batching or Terminal effects.

This is the required B37 interpretation, not a claim that the runtime projection
already implements it. At this alignment, `crates/agents/src/lib.rs`'s `prompt`
accepts live `Idle | Error` and refuses ended/closing Attempts, while
`crates/messages/src/lib.rs`'s `ended`, `on_status` and fresh-delivery scheduling
still classify by Kind. #1847 owns that projection repair and B37's runtime,
Decision-note and real-Claude observer acceptance; this alignment does not close
it. V4's existing pure transition correspondence remains unchanged; V7 must also
check the lifecycle projection against those seams.

## Modules

| Module | Change |
|---|---|
| `crates/contracts` | `Message` (with `passedFrom`), `MessageKind`, `MessageStatus`, `Held` reason, `Route`, `Delivery` (moved out of `contracts/hooks.ts`, which imports it); methods `message.send`, `message.pass`, `message.get`, `message.list`, `message.deliver`, `message.drop`, `route.set`, `route.list`, `takeover.begin`, `takeover.end`; events `message.sent`, `message.held`, `message.delivered`, `message.dropped`, `route.changed`, `takeover.changed`; `generated/` is regenerated, never edited |
| `crates/messages` (new) | stores Messages and Routes in `messages.db` of the Project data folder (N1, ADR 0004), holds Takeovers in memory, listens to `agent.status` on the bus, and types through `Agents::prompt` |
| `crates/agents` | none: delivery calls `Agents::prompt` (H11, `scenarios/control.md`), the Adapter's one input seam, so no fourth seam (P4) |
| `crates/rupd` | registers the module |
| `crates/rup` | MCP tools `message_send`, `message_get`, `message_list`, `message_pass` (B12) |
| `crates/provenance` | none: `item` is a free string, `message:<id>` joins `todo:` and `pad:` (B11) |

B32's Daemon-internal `TerminalOwners` dependency keeps `rpc::Module` unchanged. Production
uses `Agents::with_terminal_owner`: Rail then Attempts then the Message store. Reconciliation
and input recording commit together, including in-memory Takeover and input holds; SQL or
recording errors roll all of them back and publish no buffered events. A fenced receiver commits
only authoritative ending reconciliation. Ownership lookup errors do no store work. Events and
ordinary delivery scheduling follow the returned operation; delivery resolves current Status and
Message fences again, so an escaped owner id is only a scheduling hint. This touches M1, M3,
M6, M7, M9 and M10; their modeled input, ending and replay transitions stay unchanged, and
none of M1–M10 is weakened.

Bounds, from the codes `rpc::code` already has: 32 `pending` or `held` Messages per receiver (pushed digests have their own bound of 20, B22) and 8192 bytes of body (`CONFLICT` and `INVALID_PARAMS`). No new error code.

## The gates

- **P1.** No new path makes the user address an Agent instead of the Thread, or makes an Agent reachable only through the Thread. During a Takeover or unsent input, no automatically delivered Message reaches the Agent; the user's explicit `message.deliver` still wins (B6, B32).
- **P3.** Nothing here interrupts the user. A Message to the user is stored, changes no Kind and carries no signal beyond the event (B10). The Inbox UI is not in this series; when it inks a badge for Held Messages it must answer this gate itself.
- **P4.** The `messages` crate reads no vendor string; typing goes through the Adapter seam that exists.

## Escalation

Serves P1 and P3. P1: the user can still address any Agent, and a question never reaches the user through an Agent that owns the Thread's place. P1 also: a hop `held` for a Takeover waits, so a Takeover is never cut short. P3: the Escalated question is a Held Message, with no signal beyond the event, and a permission Card skips the chain, so only a valid Card interrupts. Added contract: `message.pass`, `passedFrom` on `Message`, and the Held reason `escalated`; the clock is injected, as B14's fake clock needs. A Door's hop uses the Door's own Agent, so nothing here adds a seam.

## Child digests

Serves P1, P3 and P4. P3: pushes go to a Door as `note` Messages, never to the user, and change no Kind. P1: a Door learns its children through a tool and Messages, so nothing makes the user address the Thread through it. P4: the envelope reads Status labels, Messages, Todos and Pads, never Terminal output or a vendor string. P7: a merge names the two Messages it folds and drops them with the reason `merged`, so `message.get` still opens them. Added contract: `agent.digest`, the type `Digest` and the `Reason` `merged`; the MCP tool `agent_digest` is offered to Doors only. Tool set per kind of Agent, for this PR alone: M1's tools and B12's four for every Agent, plus `agent_digest` for a Door. The numbers in B19 to B22 (512 bytes, 20 children, 8 pushes per minute, 20 held, 4096 bytes) are starting values: the Builder measures them, and a later PR may change them. The composer is `crates/messages`, which listens to `agent.status` and the Todo and Pad events; it reads no new input.

## Contract timing

The contract is not a separate PR: types and methods with no caller would be unused exports (`AGENTS.md` rule 2). It lands with the first Builder PR below, as A16's did, and rule 4 applies to that PR: the review run's approve covers it. `docs/extension-interface.md` now says `to: Actor`; Group fan-out and the `bus.route` Hook are not in this series.

## Open

Two choices came from the user's ask and are settled in `scenarios/messages.md`: Takeover starts and ends by an explicit call from the webview (B6), and a Held Message does not block a later one (B7).

## A35 retained-Agent restart

This in-process seam is for #808's Daemon coordinator, not RPC. Consent, Proof,
selection, lifecycle acknowledgements and the per-Workstream retry belong to that
coordinator. It must persist/reuse its operation key and exact Door first prompt.

```text
reserve_restart(agent, key, first_prompt?)
  → stop and restart through Agents; observe matching SessionStart
  → Door: observe first-prompt acknowledgement; record_restart_prompt(agent, key, attempt)
  → release_restart(agent, key, attempt)
```

Reserve before any selected Agent stops. Reservation waits for accepted Message
prompts to finish. It persists in `messages.db`, holds no Message by a new Reason,
and changes no Takeover. A conflicting key, changed first prompt or a missing
receiver is refused. An already stopped retained Agent can reserve its next
Attempt; its ordinary B9 ending is reconciled first, and no final Message revives. The reserved first prompt is allowed only for a Door.
There is one active reservation per retained Agent, and a completed-key tombstone
makes repeats no-ops. No caller changes outside Messages are needed until #808
uses these operations; that Issue remains open for full A35/U374 acceptance.

A reservation keeps the logical Message receiver alive while its Terminal is
replaced. Stop/error, next Attempt, delayed earlier Statuses, lag resync and Daemon
reopen preserve pending and held records and never type them. In the pure model,
`restart_event` projects these temporary Exit/Idle/Restart inputs to Busy; entering
the window first applies Busy. Final receiver removal without an explicit retirement reservation bypasses this
projection, ends the restart reservation and follows B9. Outside a reservation,
actual ending applies Exit and drops pending and Takeover-held Messages; a live
StopFailure instead applies Idle under B37. Ask-first and
escalated holds retain their Reasons; skipped Agents' Takeovers are untouched.
Question bounds pause while their receiver is reserved and restart on release;
an explicit `message.pass` remains the Actor's choice.

Release requires a live matching Attempt greater than the reserved Attempt, and
any Door note's acknowledgement to be recorded for that Attempt or an earlier
completed Attempt of the same operation. It rebinds only
pending/held records, retains their B7 ranks, and nudges an already delivery-ready
receiver (idle or B37 live error).
Final Messages are never rebound or revived. A failed restart or recording leaves
its reservation available to the same retry, including after reopening the Daemon.
On reopen, reservations replay from SQLite before reconciliation. Takeovers and
unsent input remain memory-only under B6/B32; their reserved held records wait
until recovery releases them, then become pending in their normal release order.
The coordinator must complete a successful next Attempt with the same key; a
reopen alone never releases a reservation or repeats a prompt.

`record_restart_prompt` records the exact reserved lifecycle text as one delivered
`you`/`note`, without calling Deliver/Steer, changing Iteration or rewriting earlier
Thread records. The reservation and Message id commit atomically. A Touch failure
or injected recording/publication failure can be retried: the existing note and
user's wrote Touch are reused. Successful recording emits normal `message.sent`
and `message.delivered` events once. Like other bus events, these are live signals,
not a durable subscriber log; reopening reads the Thread. The coordinator passes the acknowledged Attempt even if it has since stopped.
Recording this fact needs no live Terminal; queue release does. The lifecycle seam owns
acknowledgement/replay of the already typed prompt, so the coordinator retries
recording rather than sending it with `message.send`.

Affected laws: M1 (no second typing or note), M3 (no Takeover as an internal lock),
M4 (B7 ranks retained), M5 (final records untouched), M6 (no queue loss), M7 (final
ending remains Exit, temporary restart is Busy), M9 (durable reservation/note
replay), M10 (delivery follows coordinator release and fair Idle, including B37
live error). M2's holds stay
unchanged; M8's bound fairness additionally requires completion of the explicit
restart. `model.bend` models the reservation as a schedule projection into the
unchanged delivery transitions and the already-typed prompt as an idempotent
record journal that allocates one delivered id from the shared supply without a
Type effect. Its stored frame replays the reservation bit before interpreting
receiver loss on reopen. `PROOF.bend` checks the projection's safety,
record preservation on reserved stop/reopen, journal idempotence, at-most-once
acknowledgement, final-state disjointness and shared-id conservation. The existing
laws and predicates are unchanged; proofs cover this model, and `a35_` public-seam
and invariant replays tie its guards to Rust.


## A35 later-Agent retirement

This separate in-process operation supplies #808's coordinator. The coordinator
validates user Proof, selects later Agents and obtains Takeover consent. A skipped
Agent is never reserved or removed and keeps its Takeover. This operation supplies
no RPC, consent, lifecycle removal, rewind completion event or Thread action.

```text
reserve_retirement(agent, key)
  → if authorized: takeover.end (ends Takeover, preserves its held records)
  → Agents::retire (stop and remove from Rail; keep Worktree and branch)
  → complete_retirement(agent, key)
```

Reserve before stopping: it waits for in-flight Messages delivery, then atomically
persists the key and pauses question bounds. It changes no existing Message id,
content, status, Reason, rank or ordering, and creates no Takeover. It refuses an
empty key, a missing receiver, stale Attempt or another restart/retirement
reservation. An already stopped later Agent can reserve retirement after reconciling
its ordinary B9 ending; dropped/final records never revive. Same-key retries succeed before or after removal and completion.
Selected retirement immediately makes the receiver ineligible for new sends and
manual delivery, including while still on the Rail. Explicit drop/pass retain
their existing Actor permissions; the operation never drops or passes an archived record automatically.
An authorized `takeover.end` clears its Takeover without promoting held records.
Retirement does not itself end a Takeover; the coordinator must end a selected
Takeover before lifecycle removal. Unsent input cannot mutate retired records.

Stopping Statuses, delayed older Statuses and lag resync still reconcile receiver
generations. The durable retirement selection projects their automatic Exit/Idle/
Restart inputs to Busy and marks the receiver closed; it does not skip resync or
make a missing receiver live. Ordinary final ending still follows B9's Exit:
pending and Takeover-held Messages are dropped as receiver-gone, even if a retained
restart had been reserved. A35 retirement instead retains archived pending/held/
final records in a permanently ineligible receiver. Completed records have no
Terminal and can never be automatically typed by a later event or Attempt.

Completion requires the matching key and absence from the Rail, then durably marks
completion. It needs no SessionStart. Repeating completion is a no-op, and the
completed tombstone keeps the same operation's protection forever. A reservation,
Rail read or completion failure returns its error without releasing prior protection
or another Agent's completed work. Before removal, retry the same key and finish
lifecycle removal; after removal, retry completion. On Daemon reopen, SQLite replays
both states before receiver reconciliation. Neither reopen nor completion releases
archived holds, rebinds records or types pending Messages. Takeover/unsent memory
still vanishes under B6/B32, while archived Takeover-held Reasons remain unchanged.

Affected laws M1–M10: M1/M3 prevent typing and internal Takeover locks; M2/M4 retain
held Reasons and delivery ranks; M5 keeps final records final; M6 conserves every
record; M7 keeps ordinary receiver-gone safety and distinguishes retirement's
ineligible archive; M8 pauses automatic hop bounds for this selected receiver;
M9 replays selection/completion and records durably; M10 permits delivery only to
eligible receivers. `LAWS.bend` is unchanged: its fair-delivery cycle proves the
ordinary eligible receiver. The retirement refinement proves ineligibility,
completion idempotence, no typing on Idle, refused send/manual delivery, and record
preservation on stop, authorized Takeover end and reopen. Public `a35_` regressions
also cover lag, stale events, lifecycle/coordinator failures and same-key recovery.

#808 must compose these operations with `Agents::retire` in real-Daemon tests,
including `a35_a_later_subagent_leaves_the_rail_and_keeps_its_worktree` and
`a35_messages_todos_pads_and_checkpoints_stay`. This prerequisite leaves #808 open
for full A35/U374 acceptance.

### B6 original prompt effects

`decision.capture` returns a Prompt identity for a live Decision. A held
`decision.answer` or answer-like `terminal.write` supplies it as `identity`.
The Agent checks the Daemon run, Agent, Attempt and original Decision together
with the effect. Terminal input checks on the input thread and consumes the
identity before its physical write; direct user input invalidates it even when
H7 keeps a native Decision visible until the next Signal. These are explicit
Terminal calls, not H16's forbidden lost-Signal fallback; ordinary user writes
retain their queued acknowledgement. A refused guarded user write does not submit
Unsent input or release its held Messages. Missing capture,
replacement, dismissal, input, Attempt change and restart refuse safely; a
Daemon queue must record the refusal rather than retry without the identity.

This prerequisite integration-tests M1 (one original effect), M6 (explicit
refusal), M7 (ended Attempt) and M9 (old Daemon identities cannot replay).
M3's acquisition, M4's per-child release order and M10's release progress belong
to #1581. `proofs/messages` models Messages, not Door actions: none of its laws
or transitions changes or weakens here.

## B6 Door resize actions

Door `terminal.resize` calls during Takeover are stored in the Daemon's Project database before their receipts return. `terminal.resizeHistory` (MCP `terminal_resizeHistory`) exposes the accepting Agent's own receipts; the user sees all receipts for that Terminal, including after it is gone. The receipt's `ResizeOutcome` distinguishes waiting for Takeover, an effect in progress, completion and a drop reason. Direct user resizing retains its null result and remains available during Takeover.

The socket reader registers each authorized resize’s child order with Tokio watch channels before spawning its request, so pipelined frames cannot acquire durable ids in scheduler order. Completed requests release those channels and retire their entries. Release serializes accepted resizes for one child before any later Door resize. Before each physical effect, its in-progress outcome is committed; a crash after that cut never retries it. A restarted Daemon has no surviving Terminals or Takeovers, so unfinished resizes receive a durable drop reason, distinguishing an unavailable original Terminal from an uncertain prior effect. Completed outcomes remain completed. Terminal exit or child Attempt replacement also drops unavailable resizes; later valid resizes proceed.

This is an integration-tested B6 slice for resize only, touching M1, M3, M4, M6, M7, M9 and M10. The Message model in `proofs/messages` still models Messages, not Door resizes; none of its laws or Message exceptions changes. #1581 retains the other action paths and full B6 acceptance.

## B6 Door rename actions

Door `rail.rename` calls during Takeover return a durable `HeldAction`, rather
than the Rail node returned by an executed rename. `agent.actionHistory {id}`
(MCP `agent_actionHistory`) lists the accepting Agent's own actions on that
child; the user can read all of them, including after removal. Receipts retain
method, parameters and the same outcome tags as resize receipts. The MCP tool
keeps that JSON, so a waiting receipt is never described as an executed rename.
The wire reader captures a known Agent process's identity before waiting for
prior calls; claiming the user cannot bypass rename holds or receipt privacy.

Resizes and renames share one append-only acceptance sequence and outcome
ledger. Existing resize history migrates with its ids and outcomes intact;
`terminal.resizeHistory` keeps its existing shape and filters only resizes.
Release preserves their mixed order and checks current Reach before effect.
User renames remain immediate. Child removal or lost Reach drops the rename
with a retained reason and does not block later actions.

No Takeover survives restart. A waiting rename can still name the same
persistent child, so startup releases it. An in-progress rename is never retried:
its retained reason explains that its effect may have happened. Terminal resize
recovery retains the unavailable-original-Terminal rule. A child exit releases
valid renames through the Takeover change observer.

This slice integration-tests M1, M3, M4, M6, M7, M9 and M10. The existing Message
proofs model Messages, not held Door actions; no law or Message transition is
changed. #1581 still owns the remaining action families and full B6 acceptance.

## B6 Door Stop actions

Door `agent.stop` calls during Takeover use the same durable `HeldAction`
receipts, `agent.actionHistory`, per-child arrival order and append-only ledger
as resizes and renames. MCP preserves both waiting receipts and the null result
of an immediate Stop; it never describes acceptance as a completed stop.
Known Agent processes cannot bypass the hold by claiming the user. Direct user
Stop remains immediate.

Release rechecks Reach and the original Attempt before stopping. A Terminal
attaching to that same starting Attempt keeps its waiting Stop valid. During
pending Resume, the old displayed Terminal does not invalidate a Stop of the
resuming Attempt. The Agent checks the ordinal atomically with its Stop claim.
A removed child, an exited Terminal or a replaced Attempt produces a retained
drop reason; later actions still proceed. Stop completion waits for the Agent's
existing exit confirmation. An uncertain effect remains explained by its
recorded reason and is never automatically retried.

No Takeover or Terminal survives restart. A waiting Stop is dropped with the
original-Attempt reason rather than applied to a replacement Attempt. A Stop
recorded in progress is dropped as possibly already applied; a completed Stop
retains its completion. These are integration-tested action guarantees for
M1, M3, M4, M6, M7, M9 and M10. Message proofs and laws are unchanged. #1581
retains the remaining action families and full acceptance.

## B6 Door Interrupt actions

Door `agent.interrupt` calls during Takeover use the same durable `HeldAction`
receipts, `agent.actionHistory`, per-child arrival order and ledger as Stop.
Without a Takeover an Interrupt still runs under the shared child lock, so it
reaches an in-flight Steer; the Takeover check is made under that lock, where
`takeover.begin` cannot interleave. MCP preserves the waiting receipt and tells
the Door that acceptance is not execution.

Release rechecks Reach and claims the Escape only while the original Attempt
still runs, so a replacement Attempt's turn is never interrupted. A removed
child, an exited Terminal or a replaced Attempt gives a retained drop reason and
later actions proceed. A waiting Interrupt does not survive a Daemon restart: it
is dropped with the original-Attempt reason; one recorded in progress is dropped
as possibly applied, and none is retried. These are integration-tested guarantees
for M1, M3, M4, M6, M7, M9 and M10; Message proofs and laws are unchanged. #1581
retains the remaining action families and full acceptance.

## B6 Door prompt actions

Door `agent.prompt` calls during Takeover use the same durable `HeldAction`
receipts, `agent.actionHistory`, per-child arrival order and ledger as Interrupt.
Release rechecks Reach and writes the prompt only while the original Attempt
still runs, checked under the admission lock that claims the Terminal, so a
replacement Attempt never receives it. A prompt that finds the child busy, a
removed child, an exited Terminal or a replaced Attempt gives a retained drop or
failure reason and later actions proceed. A waiting prompt does not survive a
Daemon restart: it is dropped with the original-Attempt reason; one recorded in
progress is dropped as possibly typed, and none is retried. These are
integration-tested guarantees for M1, M3, M4, M6, M7, M9 and M10; Message proofs
and laws are unchanged. #1581 retains the remaining action families (resume,
Terminal writes, answers, rewind, reads) and full acceptance.

## B6 Door Terminal close and kill actions

Door `terminal.close` and `terminal.kill` calls on a taken-over child's Terminal
use the same durable `HeldAction` receipts, `agent.actionHistory`, per-child
arrival order and ledger as resize. Release rechecks that the child still owns
the original Terminal and that it still runs; an exited Terminal, a removed or
replaced child gives a retained drop reason and later actions proceed. A waiting
call does not survive a Daemon restart: it is dropped with the original-Terminal
reason, and none is retried. These are integration-tested guarantees for M1, M3,
M4, M6, M7, M9 and M10; Message proofs and laws are unchanged. #1581 retains
answers, rewind and reads.

## B6 Door Terminal writes

Door `terminal.write` calls on a taken-over child's Terminal use the same durable
`HeldAction` receipts, per-child arrival order and ledger as close and kill. The
bytes and any prompt identity are kept as accepted; release rechecks Reach and
Terminal ownership and writes through the same guarded path, so bytes aimed at a
prompt the user has since answered or replaced are refused and dropped with a
reason. A waiting write does not survive a Daemon restart (dropped with the
original-Terminal reason) and none is retried. The user's own writes are never held.
Integration-tested guarantees for M1, M3, M4, M6, M7, M9 and M10; Message proofs and
laws are unchanged.

## B6 Door Orders

Door `agent.setOrder` calls on a taken-over child use the same durable `HeldAction`
receipts, per-child arrival order and ledger as rename. Nothing is stored and no
Steer is typed while the Takeover lasts; an invalid Order is refused at once.
Release rechecks Reach, stores the Order and Steers the child's current Attempt,
so a replacement Attempt takes it as the child's own. A waiting Order that a
Daemon restart finds is released in order; one recorded in progress is dropped as
possibly applied, and none is retried. An Agent's own Order is never held.
Integration-tested guarantees for M1, M3, M4, M6, M7, M9 and M10; Message proofs and
laws are unchanged.

## Wake (B41 to B45)

Serves P1 and P3. An Agent asks `agent.wake {ids, any?}` to be told when all, or any, of the named Agents finish (`idle`, `done`, `error` or `needs-you`), and ends its turn instead of polling. The request is a row in `wakes` (migration 003 of `crates/messages`, saved in `project.db`), so a Daemon restart keeps it (D6). `rupd`'s `Waker` decides it from the Event log (`Bus::saved_after`, D10) and the present Kinds; a firing deletes the row and inserts the one `[wake]` note from `rupd` in one transaction, so it fires exactly once and is typed as B39 says. The note is outside B1's open bound and B22's caps. A child that enters `error` or `needs-you` always sends its parent a `[digest]` note at once (B43), which replaces B21's push for that change. There is no timer: only Events and the restart replay wake a request. The MCP tool `agent_wake` and `rup agent wake <id>... [--any]` are built from the one command list (I1), and the Brief says to wake instead of poll.

## B6 Door answers

Door `decision.answer` calls for a taken-over child use the same durable `HeldAction`
receipts and ledger. The Daemon captures the original prompt identity when it accepts
the call, so release answers that prompt only: one the user answered first, or whose
Attempt ended, is dropped with a visible reason and never answers a later prompt. A
waiting answer does not survive a Daemon restart (dropped with the Attempt reason),
and none is retried. The user's own answers are never held.
Integration-tested guarantees for M1, M3, M4, M6, M7, M9 and M10; Message proofs and
laws are unchanged.

## B6 Door Resumes

Door `agent.resume` calls on a taken-over child use the same durable `HeldAction`
receipts, per-child arrival order and ledger as Orders. Nothing starts while the
Takeover lasts. Release resumes the child if it has exited, as an exit ends the
Takeover; a child still running makes the Resume invalid, and it is dropped with that
reason. A Resume refused while the exit settles had no effect and is tried again.
A waiting Resume that a Daemon restart finds is released in order; one recorded in
progress is dropped as possibly applied, and none is retried. Integration-tested
guarantees for M1, M3, M4, M6, M7, M9 and M10; Message proofs and laws are unchanged.

## B6 Door Terminal reads

A Door's `terminal.snapshot`, `terminal.screen`, `terminal.read` and `terminal.search`
of a taken-over child's Terminal return nothing until the Takeover ends. A read has
no effect to record, so it holds no receipt: the call waits at the Daemon, then runs
behind the actions released at the end, and a Daemon restart drops the connection.
A begin that races a read is isolated by the child's effect lock. The user's own
reads are never held. Rewind has no Daemon method yet, so there is no call to hold.
Integration-tested for M3 and M4; Message proofs and laws are unchanged.
