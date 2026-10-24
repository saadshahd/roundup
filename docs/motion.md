# Motion

Accepted implementation target: U3010a–d in `scenarios/ui-motion.md`, commissioned by #3790; implementation is #3821. This document specifies the migration; it does not claim installed Motion, changed visuals, measured bundle bytes or a performance pass. M1–M11 below replace the earlier CSS engine/adoption gate. `docs/quality-target.md` and a C10 scenario heading are absent on origin/main; #3790 supplies those requirements directly.

## Motion requirements

| Requirement | Trigger and result | Observer |
| --- | --- | --- |
| M1 | Keys, keyboard list moves, tab switches and Return/Space activation: final state/focus by next frame, no timed effect, even during pointer motion | U3010b behavioral App seam plus rendered key interruption |
| M2 | Enabled pointer press: scale 1→0.97 in 100 ms; release/cancel/blur returns to 1 in 100 ms; no added background | U3010b rendered press/cancel; disabled action regression |
| M3 | Pointer Drawer, Shelf and Terminal Bubble: Motion spring, duration target 250 ms, bounce ≤0.1, retarget displayed position and velocity | U3010b continuity trace; existing focus/resize regressions |
| M4 | Pointer App-rendered menu/popover: trigger-corner scale 0.96→1 and opacity 0→1, 150 ms enter, 100 ms exit | U3010b rendered placement/exit; Ark focus/action test |
| M5 | Pointer/incoming reorder, nest, add/remove: Motion layout, 200 ms; no height keyframes | U3010c measured Solid layout probe and source review |
| M6 | Dragging: accent outline on lifted row, dashed landing slot, other rows stationary; committed drop uses M5 | U3010c rendered drag; #3540 owns shared drag behavior |
| M7 | Acknowledged non-key Todo completion: tick draws, row starts folding at acknowledgement +300 ms through M5; data/count changes immediately | U3010c fake-clock lifetime and rendered tick/fold |
| M8 | Clipboard success: activated control's label “Copied” for 1.5 s, through one helper; rejection shows failure line | U3010c fake clipboard/clock and every rendered caller |
| M9 | Kind change: 150 ms glyph crossfade; only working rotates at rest | U3010c rendered Kind transitions; P3 attention regression |
| M10 | One live preference source, no `!important`: all movement becomes 150 ms opacity fade, final geometry/focus next frame; no loop/replay; M1 still instant | U3010b/d live switch, rendered reduced variants and source review |
| M11 | Only transform/opacity interpolate; all settings resolve Tokens; every App-owned effect appears in the inventory or is deleted | U3010a/d source inventory, critic and bundle observation |

These are policy and rendered obligations except the named behavior tests in the scenarios. Tests do not assert timing curves, geometry, exact copy or computed styles. “250 ms spring” means the duration parameter, not a claim that physics ends at exactly 250 ms. Report actual settling time and overshoot.

## Accepted effects

Key source always overrides the preset to instant. Under reduced motion every non-key movement below uses the single 150 ms opacity preset, final geometry is immediate and loops stop. Static state changes stay instant. All times/scales/curves/spring parameters are Token values; this table gives targets, not permission for caller literals. The existing easing Token may supply tweens; it is no longer the only allowed curve.

| Surface / source | Purpose / family | Target preset / properties | Exit / interruption | Acceptance |
| --- | --- | --- | --- | --- |
| Enabled control / pointer down and release | Feedback / press | 0.97 scale, 100 ms each leg; transform | Release/cancel/lost capture/blur returns to 1; disabled stays still | U3010b M2; U133/U510/U3528 |
| Drawer / pointer open-close | Spatial consistency / slide | Motion spring, 250 ms, bounce ≤0.1; transform/opacity | Retarget current position/velocity; inert exit; one settled Terminal fit | U3010b M3; U5/U40/U111/U430 |
| Shelf / pointer open-close | Spatial consistency / slide and scrim | Same spring; transform/opacity | Reserved mode commits final centre width once and fits at settlement; overlay never fits | U3010b M3; U370/U371 |
| Terminal Bubble / pointer open-close | Spatial consistency / spring | Same spring; transform/opacity | Current position/velocity; existing single-bubble placement/focus | U3010b M3; U2580a amended |
| Drawer, Shelf, Bubble, Help, Leader, Switcher / key open-close | Immediate access / none | 0 | Cancel stale effects; final content/focus next frame | U3010b M1; U2580a |
| Help, Leader, Switcher / pointer | Immediate access / none | 0 | Existing immediate dismissal/focus | U2580a |
| App-rendered menu/popover / pointer open-close | Spatial consistency / corner entrance | Scale 0.96→1 plus fade, 150 ms in, 100 ms out | Resolved trigger origin follows Ark placement; exit inert; key immediate | U3010b M4 |
| Web modal and scrim / pointer | Feedback / fade | Existing 120 ms crossfade Token; opacity | Current opacity; Ark owns focus/dismissal; key immediate | U3512; U3010b/d |
| Toast / arrival | Feedback / edge lift | Existing 200 ms enter Token/easing; transform/opacity | Current displayed state; no new alert eligibility | U3793; U3010a/d |
| Toast / expiry or dismissal | Feedback / sink and fade | Existing 150 ms exit Token/easing; transform/opacity | Existing timer pauses on hover/focus/hidden window; keyboard dismissal immediate | U3791/U3793; U3010a/d |
| Toast time bar / visible timer | Time left / drain | Existing 5 s notice, 10 s failure, linear; transform | Pause with timer; reduced motion settles bar/fades instead of draining | U3790/U3791 amended by U3010d M10 |
| Permission Card / answer | Result / fade to line | Existing 150 ms fold Token; opacity | Existing identity/focus/acknowledgement; key immediate | U3795; U3010a/d |
| App rows (including Todo) / pointer or incoming add/remove, done fold, nest, committed reorder | Structural continuity / layout | Motion `animateLayout`, 200 ms; transform/opacity | Authoritative layout committed once; retarget displayed projection; removed content inert | U3010c M5; U53/U156 amended |
| Todo / keyboard mutation or list move | Immediate access / none | 0 | Final order/focus identity next frame; no retirement hold | U3010b/c M1 |
| Todo / acknowledged non-key completion | Feedback / tick then retirement | Tick revealed with transform/opacity during 300 ms hold; M5 fold follows | No animation before acknowledgement; refusal keeps row/cause; no replay on mount/switch | U3010c M7; U3770/U3771 amended |
| Inbox row / pointer or incoming expand/collapse/arrival/fold/group move | Structural continuity / layout and fade | Motion layout 200 ms; transform/opacity | Retarget projection; no height keyframes; folded content inert | U3770e amended; U3010c M5 |
| Order context parts/list / change | No accepted independent effect | Instant; remove width/opacity transitions | Data updates at once | U3010a M11 |
| Todo comment / arrival | No accepted independent entrance | Instant; delete `todo-comment-in` | Existing comment identity and scroll ownership | U3010a M11 |
| Tool-call details / pointer fold | Structural continuity / layout | Motion layout 200 ms; transform/opacity | Final geometry once; no `call-fold` height keyframes; key immediate | U3010a/c M5/M11 |
| Scrollbar thumb / pointer enter-leave | Feedback / fade | Existing 200 ms Token/easing; opacity | Retarget opacity; no interpolated color | U3747; U3010a/d |
| Lifted row and landing slot / drag hold | Direct manipulation / none | Follows pointer; no sibling displacement | Escape/outside cancels; on commit M5; dashed slot/accent outline stay | U3010c M6; #3540 |
| Glyph / Kind change | State indication / crossfade | 150 ms; opacity | Latest Kind wins; old glyph inert | U3010c M9; U446 amended |
| Working Glyph / at rest | State indication / turning arc | Existing 960 ms linear cycle; transform | Stop on Kind change, disposal or reduced motion | U446; U3010c/d M9/M10 |
| Other five Kinds, Rail leaf fold, hover, restored state / all | Immediate state / none | 0 | No arrival pulse, shake or timed hover | U446/U405/U112; U3010a |
| Copy control / acknowledged clipboard write | Feedback / own label | 1.5 s label lifetime, instant label replacement | Per-control deadline restarts; reject shows cause; cleanup cancels stale timers | U3010c M8 |

Inventory all remaining App effects during implementation; migrate each under its accepted purpose to a conforming Motion row or delete its timed effect. Do not add unrelated effects. U404's Live line and retired U105 remain removed; the five non-working Kinds remain still at rest. P3 permits no routine-progress alert. Native menus and NSAlert animation are system-owned; third-party editor/Terminal internals are not App-owned effects. M4 governs App-rendered menus/popovers and changes no native menu App seam.

## Copy controls

Read these current `apps/desktop/src/` callers and every additional `writeText` caller before migration; counts in #3790 group surfaces rather than individual buttons:

| Surface | Current file / holder | Required observation |
| --- | --- | --- |
| Order draft title/ask/done | `orders/OrderDrawer.tsx`, `OrderDrawer` | Activate each field independently; only that label changes |
| Pad recovery draft | `pads/Pads.tsx`, `Pads` | Exact clipboard payload and local success/refusal |
| Project header copies (both) | `header/ProjectTab.tsx`, `ProjectTab` | Each conditional caller retains its identity/payload |
| Reaction draft | `reactions/Reactions.tsx`, `Reactions` | Local label; failure line stays visible |
| Import failure details | `importer/ImportPanel.tsx`, `ImportPanel` | Copy details without clearing import failure |
| Rail draft | `rail/Rail.tsx`, `Rail` | Copy without sending/restoring the draft |
| Todo history | `todos/TodoHistory.tsx`, `TodoHistory` | Existing JSON payload, local acknowledgement |
| Context-menu link | `ui/contextMenu.ts`, `linkEntries` / `contextMenu` | Native menu closes normally; originating item's local label shows feedback; no menu reopening or focus trap |

Native context menus dismiss before `contextMenu` dispatches `linkEntries` actions; their copied acknowledgement therefore belongs to the originating visible item's local label, via the existing callback, for 1.5 s. Preserve U2694a/U2694c native menus; do not retain/reopen an NSMenu, change its App seam or replace it with a web menu to display feedback.

The shared helper owns clipboard acknowledgement, local success deadline, timer cleanup and stale-result protection. Owners supply payload, label and failure sink; content, permissions and focus stay with them. Existing already-confirming copies join this helper rather than retaining a second feedback lifecycle. No save/file operation becomes a clipboard action.

## Lifecycle and ownership

Latest destination wins. Persistent Motion values retain position and velocity for spring retargeting; do not restart at an endpoint or reset velocity to zero. Keys cancel movement and settle visibility/content/focus by next frame. Turning reduced motion on cancels movement/loops by next frame and allows only the 150 ms opacity fade; turning it off never replays settled motion. Its source is one live shared preference subscription/read, replacing scattered media queries and `!important` overrides.

Closure prevents new activation/submission in retained exits immediately. Already-dispatched work receives its authoritative response: P11 consumes only the submitted revision and retains newer/failed drafts with original Project/object/recipient. Project switch, reversal and disposal do not redirect submissions or duplicate RPC/focus/resize. Completion belongs to the current incarnation. Existing `ui/slideLifetime.ts`, `DrawerHost`, `createShelfFrame` and `createSortedList` are inspected before integration; retain their proven ownership responsibilities while replacing their effect engines. Ark remains responsible for interaction/focus; Motion provides animation and layout measurement. No parallel legacy engine or custom spring/FLIP implementation.

M11 permits final layout changes, not interpolation of width/height. Commit centre/reserved width and row structure once, project the previous visual geometry with Motion transforms, and reach the accepted U430/U371 endpoint without per-frame Terminal fits. Tick drawing also uses transform/opacity rather than animating path length or stroke-dashoffset. Existing Token ownership survives U2579a's curve-policy supersession. No caller embeds duration, delay, curve or spring literals.

## Library decision

Verified 2026-10-24 against installed `apps/desktop/package.json`: Solid 1.9.17, Ark Solid 5.39.3 and dnd-kit DOM/Solid 0.5.1/0.5.0; no Motion dependency yet. Solid supplies signals/batching/cleanup, Ark supplies interaction, dnd-kit supplies drag ownership; none is a spring/layout animation engine. The platform supplies WAAPI but not the requested integrated spring/velocity/layout ownership. Keep those responsibilities rather than replacing an interaction primitive.

| Candidate | Primary source and maintenance | Selection / specific gap |
| --- | --- | --- |
| Motion vanilla | [source](https://github.com/motiondivision/motion), [registry](https://registry.npmjs.org/motion), MIT, 14.6.0 released 2026-10-22; [animate](https://motion.dev/docs/animate), [motionValue](https://motion.dev/docs/motion-value), [vanilla layout](https://motion.dev/docs/layout-animations) | Selected by #3790: one engine for tween/spring/velocity/layout. Vanilla `animateLayout` has existed since 14.1.0; no React or custom measured FLIP needed. Solid still owns retained exits: vanilla has no AnimatePresence counterpart. |
| GSAP | [registry](https://registry.npmjs.org/gsap), 3.15.0 released 2026-04-13; [Flip](https://gsap.com/docs/v3/Plugins/Flip/), [license](https://gsap.com/standard-license/) | Maintained layout alternative; adding its ticker/Flip beside the mandated Motion engine violates the one-engine outcome and supplies no missing Solid content ownership. No claim that GSAP lacks layout capability. |
| AutoAnimate | [registry](https://registry.npmjs.org/@formkit%2fauto-animate), MIT, 0.10.0 released 2026-07-10; [Solid integration](https://auto-animate.formkit.com/#usage-solid) | Maintained structural animation alternative; adds another automatic layout owner/engine beside Motion rather than satisfying the one-engine requirement. Not needed because public vanilla Motion layout covers this responsibility. |
| CSS/WAAPI alone | [Web Animations specification](https://drafts.csswg.org/web-animations-1/) | Browser platform retained underneath Motion; alone requires building the spring solver/velocity retarget and layout measurement machinery the work order requires us to reuse. |

Published-export probe: in an isolated temporary directory, `npm install --no-audit --no-fund motion@14.6.0 solid-js@1.9.17`, then import `motion` in Node and inspect `animate`, `animateLayout`, `spring`, `motionValue`: all four are functions. This verifies the published API, not rendered Solid continuity. The required implementation probe renders real Solid rows through `animateLayout`, commits add/remove/nest/reorder changes, interrupts at measured 50%, and records visual geometry/identity/focus. Use Motion's scoped layout API rather than writing the measured FLIP suggested as a fallback in #3790. If a gap reproduces, record its ordinary input and trace on the implementation Issue; do not silently substitute a second engine. Recheck maintenance and built cost in that PR.

## Rendered continuity protocol

Use existing U26/rendered seams, dedicated headless session, fresh profile, dark 1512×950, final nested/owner-grouped `tree-40`, both Shelf modes, one Bubble, menus/popovers and densest failure text. Never attach to a personal browser. Record immutable base/head, seed bytes/hash, action targets, timestamped requested states/action sources, displayed bounding boxes, opacity, spring value/velocity, focus, completion/cancel and Terminal fits. Retain raw failed runs as well as passing runs.

For layers and rows, reverse/retarget when measured displacement first crosses 50%, reverse again at 50% of the return, then interrupt by key. Each normal/reversed segment contains at least two distinct intermediate samples. At the same animation time immediately before/after retarget, position differs by ≤1 CSS px and opacity by ≤0.01. Retargeted spring value and velocity match the pre-retarget Motion value/velocity (report units and sampling resolution); do not infer velocity continuity solely from two positions. Spring bounce ≤0.1 is the configured bound; record observed overshoot and settling time independently. Target presets are 250 ms spring duration, 200 ms layout, 150/100 ms menu, 100 ms press, 150 ms crossfade, 300 ms completion hold; duration observation tolerance is D9's 25%, with action/lifetime deadlines judged separately.

During drag hold, other rows stay within 1 CSS px and the lifted outline/landing slot remain; after drop affected geometry follows M5. Unaffected geometry remains within 1 px; no endpoint snap or stale removal. Keys settle by the next frame with no effect. On live reduced motion, geometry/focus settle next frame and only a 150 ms opacity fade remains; no rotation/translation/scale or completed replay. Scroll and Todo identity survive retarget/refusal.

Capture three normal-speed runs per flow, quarter-speed inspection using animation controls (not longer production Tokens), and reduced-motion variants including live changes. Include every copy caller's acknowledgement/rejection and all Kinds. `just capture U3010b` and `just capture U3010c` retain PNG/video/raw trace/Check readings under `artifacts/ux/<id>/{base,head}/`; never commit images/videos. A missing capture observer is an engineering repair, not permission to claim a pass. D2/D5/D6 pass with failure text visible and no Check regresses, using `docs/design-system.md`, **The checks**, and `apps/desktop/src/testing/checks.ts`, `runChecks`.

The critic compares observed interruption, focus, legibility and overshoot with Motion's [vanilla layout](https://motion.dev/docs/layout-animations) and [spring](https://motion.dev/docs/spring) references and cites concrete frames. This is acceptance of the selected Motion implementation; no further library-adoption approval or ease-out/spring bake-off is required. Chromium proves webview behavior and rendered geometry; it proves no WKWebView FPS. U3010d's cost/native obligations remain below.

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

## Historical observation

The pre-Motion U3010b observer recorded continuous CSS retargeting for Drawer/reserved Shelf and immediate key/reduced-motion settlement. That evidence describes the earlier implementation only. It does not demonstrate M3 velocity retention, the new Bubble/menu motion, M5's 200 ms layout, or M10's fade and cannot close the dependent migration. Native WKWebView evidence remains governed by U3010d and the recorder scenarios.
