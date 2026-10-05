Builder. `/compose sound:prime to load the taste rules, tdd to land the F6 fix red then green`

QA finding F6 against U41 in `scenarios/ui.md` (`artifacts/ux/findings.md`, sweep 2); own only `apps/desktop/src/rail/**` and `apps/desktop/src/keys/**`.

- Today `⌘1` on a selected Agent focuses the row, then the pane takes the keyboard back, because focusing a selected row selects it again.
- Test `u41_`: after one `⌘1` on a selected Agent, focus is a Rail row and stays there 300 ms later on the fake clock. Existing `⌘1` tests ran with nothing selected, which is why none caught it.
