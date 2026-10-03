# U6 Rail selection alignment

Moves: D2, D6, D10. Principle: P6 (Rail hierarchy remains readable; no Home or placement changes).

## Shape

One CSS declaration: `.rail-row .line` inherits the row's existing minimum height. Its existing flex centering then uses the full band. No new look literal or Token is needed. The band and row hit area remain 28px in this checkout; the reported screenshot's 24px band is not the current CSS height.

The U6 computed-style regression covers all six Kinds, Groups, Meta-agents, running/exited Terminals, depths 0–2, and rest/hover/selected states. It failed at every depth before the CSS change. jsdom checks the minimum centering box, not rendered geometry; browser measurements below supply rendered geometry.

## Proof

Linux Chromium, agent-browser, real App harness (`tree-40`). Both windows are 800px high. Selection is `token-rotation`; paired screenshots have the same scroll position. Before captures restore the original line minimum (`initial`) in a temporary browser stylesheet; after captures remove that override. No other application CSS differs.

| Width | Before name/glyph center offset | After | Selection movement within row | Row/hit height |
|---|---|---|---|---|
| 1280 | -6.5px (-6px for blocked) | 0px | 0px | 28px |
| 700 | -6.5px (-6px for blocked) | 0px | 0px | 28px |

34 rendered nodes measured at each width, including nested Groups and a Meta-agent. Additional after-exited measurements set an Agent to fresh done and emit a Terminal exit; all measured offsets and selection movements remain zero. Measurements use DOM rectangle centers; movement is relative to the row to exclude selection-driven scrolling.

- [1280 before](before-1280.png) / [after](after-1280.png)
- [700 before](before-700.png) / [after](after-700.png)
- Raw measurements: `before-*.json`, `after-*.json`; browser probe: `measure.txt`.
- [Failing regression](failing-test.txt)
- [Rail tests](focused-tests.txt): 319 passed.
- [Slop](slop.txt)
- Full check attempts: [interrupted](check.txt), [lint findings subsequently fixed](check-rerun.txt), [unrelated snapshot lag](check-final.txt), [serial Rust rerun](check-serial.txt).

No macOS/WKWebView verification or CI result is claimed. No PR, push, or GitHub access was performed.

Independent review found the correction sound but withheld merge approval for missing CI/PR/design-delta gates; see [review](review.txt). Required blank-line lint findings from full validation were corrected after that review.

Commit: `a4a9c79`, `Author-Agent: boxd-rail-align`. Only Rail CSS and its U6 regression are committed; proof is retained locally in this checkout.

Full `just check` is **not green**: the final parallel and serial attempts fail the unrelated `terminal::scenarios snapshot::u104_snapshot_cuts_old_history_before_the_visible_screen_at_one_megabyte` with `Lagged(206)` and `Lagged(3)` respectively. One earlier attempt passed all 636 Rust tests before finding the now-fixed blank-line lint errors. No Rust source was changed.

Remaining checks completed separately after Rust blocked the recipe: contracts-fresh, cargo machete, lint (one pre-existing perf/run.ts warning), root and desktop typechecks, slop all pass. Desktop suite: **93 files, 950 tests passed, 1 todo**. See [remaining check output](remaining-checks.txt). Local size and vocabulary checks pass against the commit parent; origin/main is absent.

Next: investigate the U104 snapshot reader lag, obtain green CI, then complete design-delta and independent merge review. This Builder commit is not merge-approved.
