# U162 / U163 captures

`base/` is `main` before #558 (the Rail with `+ agent + terminal + room`), `head/` is this branch. Each is `<seed>-<width>-<theme>.png` and `.checks.json` for `first-run`, `empty-project`, `door-stopped` and `agents-10` at 1280 and 700 wide, dark and light, headless agent-browser against `just harness`. `loop/rules.sh delta base head` reports no regressed Check.

The first head run regressed D2 on `door-stopped` (the inside line's `padding-left` was `2ch`, not a space step); the line now indents with a width, and `head/` is the run after that fix.

`longname/`: `tree-40` with its 92-character Workstream selected, 1280 and 700 wide, dark. The inside line ends 32 px inside the Rail and the Rail's `scrollWidth` equals its `clientWidth`, so nothing overflows or resizes the pane.

The inside line (`+ agent  + terminal`, in that order) shows in `door-stopped` and `longname` only, where a Workstream is selected; no other seed shows an add but `+ new Workstream`.

`ring-*.png` and `ring-*.checks.json` are the drop-ring captures of the earlier PR.

The real-Daemon run on a Rail stored with `"room"` is `artifacts/ux/A25/observer.txt`. The `bend proofs/messages/PROOF.bend` run is not made: `bend` is not installed on the runner.
