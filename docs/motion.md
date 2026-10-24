# Motion

This is the motion contract, not evidence that an implementation passes it. Acceptance is in [UI motion](../scenarios/ui-motion.md). Production keeps U2579a's `--easing-out: cubic-bezier(0.32, 0.72, 0, 1)` and existing durations. A comparison cannot authorize a new production curve, spring or duration: first merge a scenario explicitly superseding the affected U2579a, U53/U156 or layer clauses.

## Accepted triggers

Every commissioned motion names a surface, action source, purpose, family, preset, exit policy and acceptance ID below. More frequent actions get shorter motion or none. State and business actions happen when requested; no action or submit waits for motion. **Key-driven layers are instant**, including Enter/Space activation and synthetic clicks during a pointer slide (U2580a). Restore and tab switching remain instant.

| Surface and source | Purpose / family | Production preset | Exit / interruption | Acceptance |
| --- | --- | --- | --- | --- |
| Drawer (Inbox, Todo, Pad, running Agent order), pointer open/close | Spatial consistency / trailing slide | `--duration-drawer` 180 ms; `--easing-out` | Reverse from displayed position; retain only exiting content; coordinate centre width, fit Terminal once at settled size | U5, U40, U111, U430; U3010b |
| Shelf, pointer open/close | Spatial consistency / trailing slide, shadow/scrim fade | Same 180 ms and curve as Drawer | Reverse; reserved centre width coordinates and fits once; overlay sends no resize; final maximised modes keep their accepted semantics | U370/U371 in `ui-shelf-frame.md`; U3010b |
| Drawer, key open/close | Immediate access / no motion | 0 | Reach final visibility and existing focus destination next frame; cancel old completion | U2580a; U3010b |
| Shelf, key open/close | Immediate access / no motion | 0 | Centre/scrim settle together; fit once only in reserved mode | U2580a, U371; U3010b |
| Help, Leader, Switcher, Bubble, pointer or key | Immediate access / no motion | 0 | Existing dismissal/focus semantics, no retained exit | U2580a |
| Toast, new card enters | Feedback / lift from the edge; cards behind move back | `--duration-toast-in` 200 ms; `--easing-out`; transform and opacity only | Reverse path; the stack fans out on hover or focus without a timed effect of its own | U3793 in `ui-toasts.md` |
| Toast, card dismissed or expired | Feedback / fade and 8 px sink; cards behind move up | `--duration-toast` 150 ms; `--easing-out`; transform and opacity only | Reverse path; hover, focus and a hidden window hold the time and resume from what was left | U3793, U3791 in `ui-toasts.md` |
| Toast time bar, any shown card | Time left / linear drain, the one motion with no act behind it | The card's whole time (5 s notice, 10 s failure), linear; transform only; runs only while the card is visible and the window is shown | Holds still with the timer; still drains under reduced motion | U3790, U3791 in `ui-toasts.md` |
| Permission Card answered, folds to its line | Result / fade to the line | `--duration-fold` 150 ms; `--easing-out`; opacity only | Instant under reduced motion | U3795 |
| Todo insertion, completion/removal, done fold; user action or incoming mutation | Prevent jarring structural change / layout, fold height | 150 ms; `--easing-out`; shared preset required by U3010a | Retarget displayed rows; exit cannot accept actions; authoritative changes/counts remain immediate | U53; U3010c |
| Inbox row expand/collapse, arrival, answered-row fold and group move | Prevent jarring structural change / height, fade | `--duration-todo` 150 ms; `--easing-out` | Retarget displayed height; folded row is inert; none under reduced motion | U3770e |
| A Todo finishing or reopening, user tick or Agent finish the Shelf shows | Feedback / tick draw, ring pop, strike grow, bar fill, count roll | Tick 420 ms after 160 ms; ring 560 ms (`--ease-pop`); strike 620 ms after 220 ms; bar 1100 ms; count 560 ms; `--ease-todo`. None on open, Rail switch or first mount; 0 under reduced motion | Bar retargets from its displayed fill; a burst queues nothing | U3770; U3771 |
| Todo pointer reorder and U156 keyboard neighbour swap | Spatial consistency / layout | 150 ms; `--easing-out` | DragOverlay follows the pointer; real rows keep their order. Shared slot enters/exits with an interruptible opacity transition, `--duration-crossfade` / `--easing-out`; keyboard preview and reduced motion are immediate. Neighbour swaps retarget current positions | U152/U155/U156; U3010c; U3800a |
| Scrollbar thumb, pointer enters/leaves a scroller | Feedback / fade | `--duration-scrollbar` 200 ms; `--easing-out`; none under reduced motion | Reverse from the displayed colour | U3747 |
| Rail, Pad and Agent Pad drag, pointer | Direct manipulation / reorder | DragOverlay follows pointer; shared slot opacity enters/exits over `--duration-crossfade` / `--easing-out`; immediate for keyboard and reduced motion | Escape/outside cancels; release uses existing mutation | U22, U535 |
| `working` Glyph, system Kind change | State indication / turning arc | 8 × `--duration-crossfade` = 960 ms (about 1 s), linear; transform only | Stop on Kind change, disposal or reduced motion; ring stays fixed | U446 in `ui-rail.md` |
| Other five Kinds, Glyph changes, Todo feedback, Rail leaf fold | Immediate state / no motion | 0 | No pulse or exit retention | U446, U405, U35, U53 |

This table does not commission every possible effect. An unlisted effect needs accepted scenario authority before implementation.

## Retired and deferred

U404 removed the Rail Live line: no live-line crossfade. U405's leaf fold is instant: no generic Rail accordion or Done Agents fold. U446 keeps `idle`, `blocked`, `needs-you`, `error`, `done` still: no needs-you arrival pulse. U105 was retired by U467: no typing/periphery dimming. The former generic “New row” and rejected-Inbox shake rows confer no authority; no shake is commissioned without an accepted scenario. Todo structure, pointer layers, eligible Toasts and the working arc above remain distinct. P3 permits no routine-progress interruption or alert.

Springs and changed easing are deferred production choices. The comparison below is evidence work, not a gallery, renderer migration or React rewrite.

## Lifecycle and ownership

Latest destination wins. Pointer reversal starts at the displayed position; key interruption settles immediately. A pointer-opened Drawer followed by Escape is closed, exit content removed and focus restored by the next animation frame. Its old completion cannot close a later reopened Drawer. A live reduced-motion preference change settles every running movement/loop to the latest destination by the next frame, preserving content and focus; removing the preference never replays a completed effect.

Closure immediately prevents new activation/submission in retained exit content. Already-dispatched work still gets its authoritative response: P11 consumes only the submitted revision and preserves newer/failed drafts with their original Project, object and recipient. Reversal, Project change and disposal cannot redirect a submission, duplicate RPC/resize/focus effects or let stale completion act on a new incarnation. Retention is visual lifetime, never business ownership.

Audit `drawer/DrawerHost.tsx` (`DrawerHost`), `drawer/drawer.ts` (`createDrawer`), `shelf/frame.ts` (`createShelfFrame`) and `todos/sortedList.ts` (`createSortedList`) before extraction. Shared lifecycle machinery earns depth only if Drawer and Shelf actually need the same cancellation, completion and retention responsibility. Its callers declare destination, action source and preset; callers do not sequence internal timers, cancellation or retention. Focus destinations, drafts, content and business actions remain with their owners. Migrate affected callers atomically; no forwarding wrapper, parallel legacy interface, custom spring engine or generic animation DSL. Existing App/fake-App and rendered observers remain the seams.

CSS and programmatic animation resolve the same Tokens/presets, including duration, easing and reduced-motion decisions. U2579a's CSS easing source stays unique; U3010a requires consolidating the existing Todo 150 ms duration behind one Token/preset without changing its value. No `transition: all`. Prefer transform/opacity, but name and profile each required layout property: U430's coordinated centre width, U371's reserved centre width and U53's fold height are accepted exceptions, measured by U3010b/c/d. Do not delete required geometry to satisfy a transform-only slogan.

## Library decision (source check: 2026-10-20)

| Candidate | Source and maintenance evidence | Decision and concrete cost/capability |
| --- | --- | --- |
| Native CSS/WAAPI plus installed dnd-kit | [Web Animations draft, 2026-10-08](https://drafts.csswg.org/web-animations-1/); [dnd-kit releases](https://github.com/clauderic/dnd-kit/releases/tag/%40dnd-kit/collision%400.5.1), 2026-10-15; repo pins DOM 0.5.1/Solid 0.5.0 | Default production selection: CSS retargeting, WAAPI playback/cancel and existing drag ownership cover current eased paths without a second engine. WAAPI alone does not own Solid content retention or synthesize springs. Demonstrate any remaining gap before custom lifecycle extraction. |
| Motion framework-independent JavaScript | [animate](https://motion.dev/docs/animate), [spring](https://motion.dev/docs/spring), [registry release history](https://registry.npmjs.org/motion): 14.4.0 published 2026-10-19 | Selected comparison candidate for spring/interruptible playback. Published mini/hybrid sizes are 3.2/22 kB, vendor figures, not measured roundup bundle cost. React layout/presence examples do not supply Solid ownership. Adoption requires a demonstrated native gap, measured incremental built bytes and rule 7/profile evidence; reuse its engine rather than inventing springs. |
| GSAP | [docs](https://gsap.com/docs/v3/), [context cleanup](https://gsap.com/docs/v3/GSAP/gsap.context()/), [registry release history](https://registry.npmjs.org/gsap): 3.15.0 published 2026-04-13 | Maintained second candidate if another engine is required. Not selected now: timelines/context cleanup add a second scheduler while Solid exit identity and draft ownership still need integration; current paths require no timeline capability absent from CSS/WAAPI. Reconsider against a named unmet capability, not a blanket ban. |
| gpui-motion | [source](https://github.com/1bye/gpui-motion), [v0.4.0 release](https://github.com/1bye/gpui-motion/releases/tag/v0.4.0), 2026-10-19 | Quality reference for interruption, not an adoption candidate: a Rust GPUI CE/Kit crate cannot drive roundup's Solid/Tauri WKWebView DOM. Renderer replacement is outside scope. |

These are selection criteria, not permission for custom machinery when a maintained candidate covers the need. Recheck releases/capabilities and specific rejection reasons per rule 9 at implementation time.

## Bounded curve comparison

Use the existing Inbox Drawer pointer-open → close → reopen flow and one Todo pointer reorder (third open row above first, final nested owner rows). For each, record the immutable base, the repaired production ease-out path and a temporary Motion JS spring variant against exactly the same seed and mutation replies. Start from `tree-40` with the final owner/nesting design; retain the complete seed JSON/hash and IDs, including the densest case and a refused action. No candidate-only data/layout changes.

| Variant | Drawer | Todo reorder |
| --- | --- | --- |
| Crisp ease-out (production) | 180 ms; `cubic-bezier(0.32, 0.72, 0, 1)` | 150 ms; same curve |
| Restrained spring (comparison only) | Motion `type: spring`, duration 0.18 s, bounce 0.1 | Motion `type: spring`, duration 0.15 s, bounce 0.1 |

Use these duration/bounce parameters without also supplying stiffness/damping/mass. Preserve velocity on retarget where supported; record the actual engine/version and any inability to do so. Keyboard layer actions stay instant in every variant; U156 keyboard Todo swaps keep their accepted 150 ms policy. No variant delays a mutation or focus.

References: Motion's [layout example](https://motion.dev/docs/react-layout-animations) for a changing list, [spring example](https://motion.dev/docs/spring) for retargeted movement and [presence example](https://motion.dev/docs/react-animate-presence) for content remaining through exit. These React examples are quality references, not instructions to install React layout/presence components in Solid.

For each flow capture three runs per variant, normal speed and quarter-speed playback, dark 1512×950, plus reduced motion. Keep paired frames, videos, seed, action times and geometry traces under `artifacts/ux/U3010b/` or `U3010c/`, `base/` and `head/`; never commit PNG/video. Record timing in both wall and animation time; slow playback is inspection, not a performance result. The design critic cites a specific linked example and frame interval showing discontinuity, delay, overshoot or visual gain, then recommends keep/change with a reason. Duration membership alone earns no quality verdict. If spring wins, file a scenario amendment naming the clauses to supersede before shipping its curve.

## Rendered continuity protocol

Use existing U26 browser/observer seams, dedicated headless session and fresh profile; never attach to the user's browser. Record each `requestAnimationFrame` timestamp and displayed bounding box (layer leading edge/width, centre width, scrim opacity, Todo row tops/fold height), action source/time, requested destination, object/Project identity, focus and animation completion/cancel events. Keep all raw samples, including failed runs.

Reverse after measured displacement first crosses 50% of the original path; reverse again at 50% of the return path, then interrupt with Escape for the Drawer. Log actual crossing position/time, not a guessed sleep. The initial and reversed segments must each contain at least two distinct intermediate samples; an endpoint-only trace cannot pass. Quarter-speed inspection uses browser animation playback controls, not longer production Tokens. Normal-speed runs remain separate.

At a pointer retarget, pause the browser animations at the observed current time and read geometry immediately before and after applying the new destination at that same animation time. Each retained edge/top/width differs by at most 1 CSS px; scrim opacity differs by at most 0.01. Resume and sample each frame to the latest endpoint. This is position continuity, not a promise of continuous velocity for eased transitions. No snap to an old endpoint or stale removal is allowed. Unaffected Todo rows stay within 1 px; settled targets are within 1 px/0.01 opacity, and ease-out finishes in 180 ±45 ms for layers, 150 ±50 ms for Todos, measured in animation time. Spring comparison reports settling time and maximum overshoot separately; it does not redefine D9. Direct dragging is unscored for timed continuity.

Key interruption and reduced motion deliberately settle rather than interpolate; their next-frame destination/focus rule supersedes the continuity tolerance for that instant. Record failure if the normal-speed observer misses its first-frame deadline; do not retry until a late success passes. Geometry and feel are rendered/design-critic evidence, never assertions in behavioral unit/UI tests.

## Native performance protocol

U3010d requires actual WKWebView evidence. Pin an isolated Project fixture derived from the final `tree-40` layout: forty Rail nodes, eight open Todos nested to depth three across two owners, four Pads, ten visible working Glyphs and one selected live Terminal. Retain fixture bytes/hash and action targets; use identical bytes for base/head. At 1512×950 in dark, run Terminal output at exactly 64 KiB/s: 64 lines of 128 bytes including CRLF every 125 ms, monotonic sequence numbers, no escape sequences. Retain generated/received byte counts and batch timestamps.

Warm up 10 s, then record 60 s per workload, three alternating base/head pairs on the same Mac, release profile, hardware, viewport, fixed display refresh, power mode and thermal state, without concurrent builds:

1. Layers: every 2 s perform the 50%-displacement pointer open/close/reopen and key interruption sequence, alternating Drawer and Shelf; include reserved and overlay Shelf cases in separately identified runs.
2. Todos: every 2 s cycle insertion, completion/removal, done-fold toggle and pointer reorder, one action per cycle, preserving final owner/nesting contracts. Repeat on restored fixture; do not complete away the workload.
3. Glyphs: all ten working arcs run while the same Terminal load continues; then stop load, settle all motion and observe 30 s idle cleanup. Repeat a reduced-motion run for each workload, toggling during movement at the measured halfway point.

Record source commits, binary hashes, build command/profile, macOS/hardware/WebKit versions, display Hz, power/thermal state, load per CPU, tool/version/settings, fixture hash, exact actions and errors. Baseline is the parent before implementation, not a newly relaxed baseline. For each workload retain raw scheduler intervals and Web Inspector style/layout/paint records, with p50/p95/p99, long-frame count/rate and maximum; report compositor/presentation stages only when the selected native instrument actually exposes them for this WKWebView. [WebKit Timelines](https://webkit.org/web-inspector/timelines-tab/) exposes browser frame work, not proof of physical display presentation. An Instruments trace must identify its actual supported stages; unsupported stages are “unmeasured”, never zero/pass.

Predeclared frame thresholds: let P = 1000 / fixed refresh Hz ms (16.67 at 60 Hz, 8.33 at 120 Hz). For each valid run, scheduling interval p95 ≤1.5P, p99 ≤3P; long interval means >1.5P and at most 5% of intervals; none >50 ms. These gate scheduling only. Presented-frame evidence, when available, uses the same thresholds on its own samples, never inferred from callbacks. No metric's paired median regresses >10% (0.001 ms timing resolution floor; long-interval rate floor one sample/N). All three head runs must meet absolute thresholds; never pool away a failing run. Presentation unavailable means native smoothness remains unmeasured and U3010d incomplete, even if scheduling passes. Diagnose style/layout/paint, Terminal fit and main-thread cost before an engine change.

An invalid run (missing metadata/raw stages, changed fixture/load, occluded/napped window, refresh/power change, concurrent build, incomplete action sequence or insufficient instrument resolution for P) is observer error, retained and repeated, never a pass. Apply `docs/perf.md`'s existing load/noise rules: a busy latency comparison is visibly skipped and cannot complete acceptance; hard limits and memory gates still apply. No threshold/baseline relaxation or raised timeout.

| Result | Exit code if automated | Meaning |
| --- | --- | --- |
| Every required stage and workload measured and passing | 0 | Acceptance evidence complete |
| Valid sample misses budget/continuity or cleanup | 1 | Diagnose and repair, retain failure |
| Unsupported/unmeasured stage, invalid run or unavailable native observer | 2 | Incomplete evidence; no smoothness claim |

Rule 7 remains cold start <300 ms, keystroke-to-render <16 ms p95, ten idle Agents <150 MB extra RSS, and the existing >10% regression rule. `just perf` measures Daemon work; `just perf-keystroke` measures Terminal echo, neither proves motion FPS. Preserve high-refresh reporting. Native observation is only on the owner's explicit Mac instruction through the existing request path; `ROUNDUP_ALLOW_WINDOW=1` remains restricted exactly as `docs/perf.md` requires. This specification runs no native window or OS input. Linux/Chromium results never substitute for WKWebView or imply 60/120 displayed FPS.

## U3010b result

`just capture U3010b` runs `drawer/u3010b_rendered.test.ts` on `tree-40`, dark 1512×950: the Inbox Drawer (pointer open, reversal at 50% displaced, reopen at 50% of the return, Escape) and the reserved Shelf (close, reversal at 50%), each normal, quarter-speed (`Animation.playbackRate` 0.25) and reduced-motion, three runs each, raw timestamped boxes, focus and content in `artifacts/ux/U3010b/head/` (untracked). Observed: each retarget starts at the displayed edge (0 px difference at the same animation time, tolerance 1 px); normal runs hold 5 and quarter-speed runs 12 distinct intermediate Drawer samples; one frame after Escape the Drawer is closed with no exit content and focus on the opening control; reduced motion jumps to the destination with no intermediate sample.

Decision: keep the crisp ease-out. The Motion spring variant was not run, so there is no measured gain to cite and no scenario amendment is filed; the library decision above stands, and the lifecycle is one small owner, `ui/slideLifetime.ts`, used by `DrawerHost` and `createShelfFrame` (no maintained library owns Solid exit retention). Native WKWebView evidence stays pending with U3010d until the Mac observation passes.

### U3512 shared web modals

| Surface | Source | Purpose | Property | Preset | Exit |
| --- | --- | --- | --- | --- | --- |
| Remaining web modal panel and scrim | Pointer | Soft appearance/disappearance | Opacity only | `--duration-crossfade` (120 ms), `--easing-out` | Same fade; retained content inert; focus/keyboard ownership released immediately |

`ui/Dialog.tsx` reads pointer modality from `@zag-js/focus-visible`; the key registry marks modifier chords as keyboard input too. It reuses `createSlideLifetime` for completion, cancellation and reversal; CSS transitions retarget from current opacity. Key open/close, recovery and reduced motion are instant. Enabling reduced motion during a fade settles it immediately. No submit or RPC waits for a fade. Ark owns modal focus containment and dismissal. NSAlert motion is system-owned; anchored panels and Terminal Bubbles retain their previous motion contracts.
