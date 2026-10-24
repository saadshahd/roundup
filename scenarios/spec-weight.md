# Specification weight

Module: `scenarios/` except `loop-*.md`, and `GLOSSARY.md`. Source: #3793. Ids: SW1, SW2.

Existing checks on main:

1. `scenarios/README.md`, Scenarios: scenario ids identify acceptance; work orders live in GitHub Issues.
2. `loop/rules.sh`, `scenario_ids`: scenario headings have unique ids.
3. `loop/rules.sh`, `avoid_phrases` and `vocab`: Avoid words govern public names, with the vendor Adapter exception.
4. `loop/rules.test.sh`, vocab section: clean names pass, forbidden Rust/TS names fail, only the vendor Adapter is exempt, and a missing glossary fails loudly.
5. `justfile`, `check`: the required Rust and web checks remain the regression boundary.

All then-clauses below are policy, observed by independent review of the implementation diff, reproducible audit output and required checks. Add no test that merely rereads these policies. Product behavior and existing tests remain unchanged except the vocabulary-reader fixture required below.

**SW1 only unreferenced retired or unproved clauses are removed.** Given the implementation's recorded `origin/main` commit, when a script inventories and deletes clauses in the module, then:

- Inventory every id heading, its file, heading text and first nonblank content line. A clause runs from its id heading to the next id heading or end of file; intervening section headings are preserved. Inspect retired file titles too. Separate whole-clause retirement from retirement of only a subclause, an old test or an old implementation. A word-count or raw occurrence count is not a clause count.
- Publish the exact candidate list and count, the script and its command in the PR body. A whole-clause retired marker is a heading declaring that clause retired, or first content beginning `Retired by` (ignoring Markdown emphasis). A partially retired clause keeps all live acceptance. Retired ids are never reused.
- Before deletion, search each candidate id, case-insensitively and with identifier boundaries, in all tracked files outside that candidate's own block. Include lowercase test-name prefixes followed by `_`, not just Rust `fn` declarations: Rust, TS/TSX and shell tests count. Include other scenarios, frozen scenarios, `docs/`, code comments and glossary citations. A reference inside another proposed deletion can disappear only when both blocks are deleted; audit the resulting tree again.
- A cited id stays. The PR body lists it under **kept, with the citer**, giving the file and matching text. A surviving test or Proof keeps its clause even if the clause is marked retired. Do not delete, rename or weaken a test or rewrite a live citer merely to make a clause eligible. This retention requirement takes precedence over removing every retired marker.
- An unmarked clause is deleted only when its block has no `Proof:` line, no live test named for its id and no surviving reference from another scenario or any other tracked file. For each such deletion, the body gives the id, exact grep commands and their no-match results. A differently named test explicitly cited by the clause is also checked before declaring it unproved. Unknown evidence is a keep pending investigation, not permission to delete.
- Every deleted id appears exactly once in a single physical line headed `Retired ids:` in `scenarios/README.md`, including unmarked deletions. Retired-only files may be removed when nothing cites their path; otherwise retain the linked file and replace its retired blocks with a link to the README list. Preserve live clauses, module declarations for surviving ids and all surviving links.
- For every deleted id, publish a final `git grep` command and result showing the README list is its only remaining tracked occurrence, including test prefixes. Search ids individually with boundaries so a shorter id cannot match a longer id. The final audit has no eligible retired or unproved clause left, no unresolved evidence error, no duplicate deleted id and no dangling id or file reference. Cited retired clauses appear only in the keep report, not the README deletion list.
- Use a script for inventory and deletion, and record per-clause decisions; a script error must leave the source unchanged. The audit covers every clause, not only the initial retired candidates. Counts and evidence are recomputed on the final head after integrating main.

**SW2 the glossary is a compact term table with working vocabulary enforcement.** Given the existing glossary and its readers, when definitions move out of the glossary, then:

- `GLOSSARY.md` contains tables with exactly `Code term`, `User label`, `Avoid` columns, one row per term, optional group headings and a short introduction; `wc -w GLOSSARY.md` reports fewer than 3,000 words. Rows hold names and labels, not definitions. Preserve existing code spellings, explicit user-label mappings and the meaning and scope of Avoid entries; this is no authorization to rename public identifiers or change App copy.
- Every removed definition, including subsidiary definitions and behavioral qualifications, has a row in the implementation PR body: original term/paragraph, destination scenario id and heading, or an explicit reason for dropping it. Move a retained definition to the existing scenario that introduces that term. A definition already stated there can be dropped as duplication with its destination cited. A retired concept or general programming concept can be dropped with that reason and evidence; an unmatched live domain definition is preserved in its introducing scenario, not silently discarded or collected in a replacement glossary document.
- Scenario moves preserve the definition's meaning and acceptance. They introduce no new RPC, type, App seam, Message law or product behavior. Readers follow existing scenario ownership; a relocation updates any heading link it invalidates in the same PR.
- Update `loop/rules.sh`'s `avoid_phrases` reader and its existing vocab fixtures in `loop/rules.test.sh` atomically with the table. Preserve the effective forbidden phrases, case normalization, multiword-to-snake conversion, plural matching, public-name scan and vendor Adapter exception. The reader fails loudly for a missing glossary or missing/empty Avoid data; it cannot silently disable rule 6.
- Exercise that boundary using the existing vocab test harness: a clean table passes; a forbidden public Rust name and a TS export fail; a multiword Avoid entry is enforced; the Adapter exemption remains restricted; absent/empty Avoid data fail. Amend existing fixtures rather than add a second test proving the same case. `loop/rules.sh vocab` and `loop/rules.test.sh` pass on the final tree.
- Resolve the existing citations in `apps/desktop/src/copy.ts`, `apps/desktop/src/ink/glyph.ts`, `apps/desktop/src/keys/Keys.tsx`, `apps/desktop/src/drawer/DrawerHost.tsx` and `loop/builder.py`. A citation to a definition or ordering moves to its introducing scenario; a citation to a code term may still point to its table row. Keep every fact those comments rely on, including Kind urgency order. No behavior change to these callers is authorized.
- Research the installed dependencies and platform before adding audit machinery; consider `markdownlint-cli2` and `remark` for Markdown structure/table checks. The implementation's Reuse section supplies the selection or specific rejection evidence required by rule 9. A one-off script and its output can be carried in the PR body; this specification requires no permanent parser or new module.

## Observers

The implementation records the commands, output and status below. For the one-off audit script, publish one row per failure, with id, file, reason and exit code; do not reduce failures to a count. Grep exit 1 means no matches, exit 2 means an error and never evidence of absence.

| Command or observer | Pass | Failure |
|---|---|---|
| Reproducible clause/definition audit script, comparing the recorded base to the final head | Exit 0; all deleted ids, kept citers and moved/dropped definitions accounted for | Exit 1 per reported missing evidence, residual citation/eligible marker, duplicate id, lost definition or broken link; exit 2 for unreadable input or parsing/search error |
| `wc -w GLOSSARY.md` and table inspection | Count below 3,000 and SW2's three columns/rows | Count at least 3,000 or nonconforming table |
| `loop/rules.sh scenario-ids` | Exit 0 | Exit 1 with duplicate id and files |
| `loop/rules.sh vocab` | Exit 0 | Nonzero with forbidden name or missing Avoid data |
| `loop/rules.test.sh` | Exit 0 | Nonzero with failing fixture |
| Required `check` on the implementation head | Pass | Failed required check; no weakened check or skipped test |

Specification completion is independent review of SW1/SW2 against #3793 and the numbered checks above; implementation remains in a dependent work Issue until these observers pass. The loop's Builder runs the prescribed lints and affected tests; CI's `check` supplies the full `just check` result. Frozen loop machinery and skill files remain unchanged by this specification. The explicitly requested vocabulary-reader migration belongs with implementation; any still-required ownership authorization is resolved on its work Issue before that edit.
