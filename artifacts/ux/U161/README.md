# U161 rendered observer

Base: `a578b4c` (main before U161's styles). Head: `28faa6e` (main with #546). Chromium headless on Linux, `?seed=door-stopped` served by `just harness`, at 1280x800 and 700x800 in light and dark. Each `.checks.json` is the complete `window.__checks()`; `.boxes.txt` holds the region and control boxes. Linux captures only; WKWebView stays the user's `just app` observation.

`loop/rules.sh delta base head` exits 0 (`delta.txt`, 40 comparisons): no Check that passed on base regressed.

## Read on head, all four sizes and schemes

- Rail `border-right` 1 px `--hairline`; Shelf `border-top` 1 px `--hairline`; the only other bordered nodes are the Drawer (closed here) and the thread composer's own field.
- Rail and Shelf pad 16 px (`--space-4`) leading and trailing; the Rail's rows share one left edge.
- Add row: `column-gap` 8 px. Centre group: `row-gap` 12 px, `start Door` 28 px high in `--accent`; the group's box centre equals the pane's at 1280x800 light (678.4, 261.5).
- Every SVG is 16 px, stroke 1.75, round joins (10 of 10).

## Not met

- D1, D5 (light), D6, D7 and D10 fail on this screen exactly as on base, so "passes D1 to D10" is not shown; D6 reports `missing: hover`, D7 `pressed: 0`.
- In dark the emulator paints white (`terminalTheme.background` `#ffffff`), a slab behind the `Door stopped` card at both sizes; present on base too. Filed as #549, which blocks #355.
- The thread composer's side margin is 12 px, not 16; it is outside the state group.
