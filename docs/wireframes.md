# Wireframes

ASCII, Tufte lens, mouse-first (no shortcut or palette layer in the MVP). Six critique iterations produced these; the corrections below were applied afterwards and win over anything drawn.

## Visibility rule

A detail is visible by default iff not seeing it blocks an action the user must take; otherwise it appears on interaction.

Applied corrections (the drawings predate some of them):

- **Live line** (the second line under a rail row) is hidden by default. It shows on hover or selection, and unprompted when kind is `blocked`, `needs-you` or `error`. Every wireframe draws it on every row for illustration only.
- **Provenance letter** (last toucher) shows on hover or selection only. Every wireframe draws it always-on for illustration only.
- Removed from earlier drafts: sparkline column, `lead→` arrows, separate prompt row, band above the prompt, Overview grid, command palette and shortcuts (superseded: U30, U32 and U33 in `scenarios/ui.md` add chords).
- Collapsed by default: done todos and agents, pad bodies, detail, history, inbox.

## Ink vocabulary (one set on every screen)

```
STATUS   one glyph per row = most urgent Kind. The same six marks are used in rail, shelf, inbox, history.
  ●  needs-you   bold + amber   ┐ the only ink in the app. In these drawings the name is in CAPITALS
  ✕  error       bold + red     ┘ to stand for bold + colour; the app never uppercases.
  ⏸  blocked     grey
  ○  working     light grey
  ·  idle        light grey     (for todos: open, nobody on it)
  ✓  done        lightest grey; folds into one "✓ n done" line

TREE
  indent 2   child of the row above. Nesting is shown by indent only, never by a box.
  ▾          plain Group with no Agent behind it; promoting replaces ▾ with a status glyph
  ───        drop target while dragging; its left end is the depth the row lands at
  ›          selected row (a faint grey band in the app)

PADS AND PROVENANCE
  ◈  ◇       agent-owned / yours; click the glyph to flip it
  A  a       last toucher wrote (bold) / only read (light). Y / y = you.

OTHER
  │          where the terminal's own background starts (a change of ground, not a drawn rule)
  ▪          this module owns this hook (extensions)
  > █        claude's own prompt inside the xterm; nothing else in the app takes typed input
  stop, close, history, export .md, promote   right-aligned lower-case words = click targets, light grey
  ci red     a Chip: light text after the name, no pill, never changes the glyph
  toast      one dim line at the window bottom, for extension errors only
```

## Screen 1 — Main workspace

Three columns, no borders drawn. Rail left, terminal centre at full height, shelf right (todos over pads), open by default. Widths: Rail 20% (252 to 320 px), Shelf 14% (160 to 280 px), terminal at least 690 px (80 columns); under 1102 px the shelf moves under the rail (U23, U24).

```
roundup   payments-api                                                                     inbox 4

  ○ checkout                    │ auth-refactor  working 12m       stop │ todos                  +
    planning: split #7 into th… │                                       │ ○ #3 refresh tokens    A
›   ○ auth-refactor             │ > [from checkout 14:02] rebase on     │ ○ #4 migrate users     R
      editing src/auth/token.ts │   main once #4 lands                  │ ⏸ #5 session store     s
    ○ token-tests               │                                       │      waits on #4
      vitest auth/  42 of 118   │ Read src/auth/refresh.ts              │ ⏸ #10 session tests    t
    ⏸ session-store             │ Update src/auth/token.ts   +14 -6     │      waits on #5
      waits on #4 migrate       │   rotateRefresh() now returns         │ ○ #7 split checkout    C
  ▾ migrate                     │   { token, expiresAt }                │ · #6 rate limits       y
    ○ rename-cols               │ Bash pnpm vitest auth/token           │ ✓ 9 done
      writing 0042_rename.sql   │   18 passed                           │
    · backfill                  │ Update src/auth/session.ts  +3 -1     │ pads                   +
      idle 6m                   │ Read src/auth/token.test.ts           │ ◈ auth-notes           A
  ○ gateway                     │                                       │ ◈ migrate-plan         r
    reading routes/v1/*.ts      │                                       │ ◈ fixtures-log         W
  ○ webhooks                    │                                       │ ◇ release-checklist    Y
    replaying 120 fixtures      │                                       │
  ○ docs                        │                                       │
    writing docs/auth.md        │                                       │
  · perf-probe                  │                                       │
    idle 22m                    │                                       │
  ○ npm run dev                 │                                       │
    ready on :3000              │                                       │
  ✓ 2 done                      │                                       │
                                │                                       │
  + agent  + terminal  + group  │ > █                                   │
```

Detail, a pad, history and the inbox open by widening leftward over the terminal. The terminal is never resized, so Claude's screen does not reflow; the rail stays visible. Clicking the sender tag `[from checkout 14:02]` flips that Route between auto and ask-first.

### Screen 2 — One agent needs you and one has errored, among 12 agents
```
roundup   payments-api                                                                     inbox 4

  ○ checkout                    │ auth-refactor  working 12m       stop │ todos                  +
    planning: split #7 into th… │                                       │ ○ #3 refresh tokens    A
›   ○ auth-refactor             │ > [from checkout 14:02] rebase on     │ ○ #4 migrate users     R
      editing src/auth/token.ts │   main once #4 lands                  │ ⏸ #5 session store     s
    ○ token-tests        ci red │                                       │      waits on #4
      vitest auth/  42 of 118   │ Read src/auth/refresh.ts              │ ⏸ #10 session tests    t
    ⏸ session-store             │ Update src/auth/token.ts   +14 -6     │      waits on #5
      waits on #4 migrate       │   rotateRefresh() now returns         │ ○ #7 split checkout    C
  ▾ migrate                     │   { token, expiresAt }                │ · #6 rate limits       y
    ○ rename-cols               │ Bash pnpm vitest auth/token           │ ✓ 9 done
      writing 0042_rename.sql   │   18 passed                           │
    · backfill                  │ Update src/auth/session.ts  +3 -1     │ pads                   +
      idle 6m                   │ Read src/auth/token.test.ts           │ ◈ auth-notes           A
  ● GATEWAY                     │                                       │ ◈ migrate-plan         r
    asks: keep v1 routes?    4m │                                       │ ◈ fixtures-log         W
  ✕ WEBHOOKS                    │                                       │ ◇ release-checklist    Y
    exited 137, out of memory   │                                       │
  ○ docs                        │                                       │
    writing docs/auth.md        │                                       │
  · perf-probe                  │                                       │
    idle 22m                    │                                       │
  ○ npm run dev                 │                                       │
    ready on :3000              │                                       │
  ✓ 2 done                      │                                       │
                                │                                       │
  + agent  + terminal  + group  │ > █                                   │

  tidy-sort  rail.sort threw, skipped
```
The glyph column is one vertical scan line, and only two of the 12 rows carry ink; the order stays manual and nothing jumps. `ci red` is a chip added by the ci-watch module's agent.status hook: CI has failed, but the glyph stays ○. The dim toast reports a hook that threw and was skipped, so the rail kept its order.

### Screen 3 — Inbox drawer
```
roundup   payments-api                                                                     INBOX 5

  ○ checkout                    │ auth- │ inbox                                              close
    planning: split #7 into th… │       │
›   ○ auth-refactor             │ > [fr │ time   from           item
      editing src/auth/token.ts │   mai │ 14:33  token-tests    ● held for auth-refactor
    ○ token-tests        ci red │       │                         "refresh race: add a mutex?"
      vitest auth/  42 of 118   │ Read  │                         deliver   drop   always deliver
    ⏸ session-store             │ Updat │ 14:22  auth-refactor  ◈ auth-notes changed, +12 lines
      waits on #4 migrate       │   rot │ 14:20  checkout       3 messages, last "split #7 in 3?"
  ▾ migrate                     │   { t │ 14:05  token-tests    "refresh race is flaky, see pad"
    ○ rename-cols               │ Bash  │ 13:58  webhooks       ◈ fixtures-log changed, +40 lines
      writing 0042_rename.sql   │   18  │ 13:52  lint-fix       · #6 rate limits unblocked
    · backfill                  │ Updat │ 13:40  docs           2 messages, last "auth.md drafted"
      idle 6m                   │ Read  │
  ● GATEWAY                     │       │
    asks: keep v1 routes?    4m │       │
  ✕ WEBHOOKS                    │       │
    exited 137, out of memory   │       │
  ○ docs                        │       │
    writing docs/auth.md        │       │
  ✓ 2 done                      │       │
                                │       │
  + agent  + terminal  + group  │ > █   │
```
- The drawer slides in from the right edge, over the shelf and the terminal; the rail stays visible.
- The badge counts rows that are new since the drawer was last opened (the top 5) and is inked while anything is held.
- Held rows never fold, because each needs its own deliver or drop. Other repeats from one sender fold.
- Clicking a `from` name flips that route between auto and ask first.

### Screen 4 — Todos (list, blockers, dependency, detail with provenance)
```
roundup   payments-api                                                                     inbox 4

  ○ checkout                    │ auth- │ todos                                     + todo   close
    planning: split #7 into th… │       │
›   ○ auth-refactor             │ > [fr │   ○ #3  refresh tokens            auth-refactor  A
      editing src/auth/token.ts │   mai │   ○ #4  migrate users table       rename-cols    R
    ○ token-tests        ci red │       │ › ⏸ #5  session store on redis    session-store  s
      vitest auth/  42 of 118   │ Read  │          waits on #4 migrate users table
    ⏸ session-store             │ Updat │   ⏸ #10 session tests             token-tests    t
      waits on #4 migrate       │   rot │          waits on #5 session store
  ▾ migrate                     │   { t │   ○ #7  split checkout flow       checkout       C
    ○ rename-cols               │ Bash  │   · #6  rate limits                              y
      writing 0042_rename.sql   │   18  │   ✓ 9 done
    · backfill                  │ Updat │
      idle 6m                   │ Read  │ #5  session store on redis                 ⏸ blocked 40m
  ● GATEWAY                     │       │ on          session-store
    asks: keep v1 routes?    4m │       │ waits on    #4 migrate users table      ○ rename-cols
  ✕ WEBHOOKS                    │       │ blocks      #10 session tests           ⏸ token-tests
    exited 137, out of memory   │       │             + blocker
  ○ docs                        │       │ created     you 13:10
    writing docs/auth.md        │       │
  · perf-probe                  │       │ Move the session cache from memory to redis.
    idle 22m                    │       │ Keep the TTL from src/auth/token.ts (pad auth-notes).
  ○ npm run dev                 │       │
    ready on :3000              │       │ last  s  session-store read 14:12                history
  ✓ 2 done                      │       │
                                │       │
  + agent  + terminal  + group  │ > █   │
```
A dependency is written as a sentence, not drawn as an arrow, and it uses the rail's own words ("waits on #4"). In the detail, each blocker shows its own live glyph. The letter at the end of a row is the last toucher.

### Screen 5 — Scratchpads
```
roundup   payments-api                                                                     inbox 4

  ○ checkout                    │ auth- │ pads                                       + pad   close
    planning: split #7 into th… │       │
›   ○ auth-refactor             │ > [fr │ › ◈ auth-notes          auth-refactor   14:22    A
      editing src/auth/token.ts │   mai │   ◈ migrate-plan        rename-cols     13:50    r
    ○ token-tests        ci red │       │   ◈ fixtures-log        webhooks        13:58    W
      vitest auth/  42 of 118   │ Read  │   ◇ release-checklist   you             12:10    Y
    ⏸ session-store             │ Updat │   stored in app                       store as .md files
      waits on #4 migrate       │   rot │
  ▾ migrate                     │   { t │ ◈ auth-notes   owned by auth-refactor         export .md
    ○ rename-cols               │ Bash  │
      writing 0042_rename.sql   │   18  │ ## refresh rotation
    · backfill                  │ Updat │ - rotate on every use; old token valid 30 s
      idle 6m                   │ Read  │ - race: two refreshes within 5 ms, see #3
  ● GATEWAY                     │       │ - TTL lives in src/auth/token.ts, not config
    asks: keep v1 routes?    4m │       │
  ✕ WEBHOOKS                    │       │ ## open
    exited 137, out of memory   │       │ - keep v1 /token route? gateway is asking you
  ○ docs                        │       │
    writing docs/auth.md        │       │ last  A  auth-refactor wrote 14:22               history
  · perf-probe                  │       │
    idle 22m                    │       │
  ○ npm run dev                 │       │
    ready on :3000              │       │
  ✓ 2 done                      │       │
                                │       │
  + agent  + terminal  + group  │ > █   │
```
Clicking ◈ makes a pad yours (◇). `store as .md files` flips the project-wide setting, which is the same switch as in Project settings. `export .md` writes one file, whatever that setting is.

### Screen 6 — New agent or new terminal
```
roundup   payments-api                                                                     inbox 4

  ○ checkout                    │ new-agent  idle  just now        stop │ todos                  +
    planning: split #7 into th… │                                       │ ○ #3 refresh tokens    A
    ○ auth-refactor             │ Claude Code 2.1.4                     │ ○ #4 migrate users     R
      editing src/auth/token.ts │ ~/repos/payments-api                  │ ⏸ #5 session store     s
    ○ token-tests               │                                       │      waits on #4
      vitest auth/  42 of 118   │                                       │ ⏸ #10 session tests    t
    ⏸ session-store             │                                       │      waits on #5
      waits on #4 migrate       │                                       │ ○ #7 split checkout    C
  ▾ migrate                     │                                       │ · #6 rate limits       y
    ○ rename-cols               │                                       │ ✓ 9 done
      writing 0042_rename.sql   │                                       │
    · backfill                  │                                       │ pads                   +
      idle 6m                   │                                       │ ◈ auth-notes           A
  ○ gateway                     │                                       │ ◈ migrate-plan         r
    reading routes/v1/*.ts      │                                       │ ◈ fixtures-log         W
  ○ webhooks                    │                                       │ ◇ release-checklist    Y
    replaying 120 fixtures      │                                       │
  ○ docs                        │                                       │
    writing docs/auth.md        │                                       │
  · perf-probe                  │                                       │
    idle 22m                    │                                       │
  ○ npm run dev                 │                                       │
    ready on :3000              │                                       │
› · new-agent                   │                                       │
    waiting for first prompt    │                                       │
  ✓ 2 done                      │                                       │
                                │                                       │
  + agent  + terminal  + group  │ > █                                   │
```
```
after + terminal, one click       after you type npm run dev        after the first prompt

› · zsh                           › ○ npm run dev                   › ○ fix-refresh-race
    ~/repos/payments-api              ready on :3000                    reading src/auth/token.ts
```
`+ agent` is one click. The row appears at the root, or inside the selected group or meta-agent; claude starts and focus moves to its terminal. There is no form and no prompt box. The agent's name comes from its first prompt, and a terminal's name comes from its first command; double-click to rename.

### Screen 7 — Meta-agent: group, promote, drag to nest / unnest / reorder, meta-agent stopped
```
1  group (default)                2  promoted, one click            3  drag perf-probe: nest

▾ migrate              promote    ○ migrate                         ○ migrate
  ○ rename-cols                     assigning #4 to rename-cols       assigning #4 to rename-cols
    writing 0042_rename.sql         ○ rename-cols                     ○ rename-cols
  · backfill                          writing 0042_rename.sql           writing 0042_rename.sql
    idle 6m                         · backfill                        · backfill
· perf-probe                          idle 6m                           idle 6m
  idle 22m                        · perf-probe                        ────────────  · perf-probe
                                    idle 22m

4  drag backfill: unnest          5  drag perf-probe: reorder       6  meta-agent stopped

○ migrate                         ──────────────  · perf-probe      ✓ migrate
  assigning #4 to rename-cols     ○ migrate                           stopped 14:40
  ○ rename-cols                     assigning #4 to rename-cols     ○ rename-cols
    writing 0042_rename.sql         ○ rename-cols                     writing 0042_rename.sql
──────────────  · backfill            writing 0042_rename.sql       · backfill
· perf-probe                        · backfill                        idle 6m
  idle 22m                            idle 6m                       · perf-probe
                                                                      idle 22m
```
`promote` is a light word that appears on hover. Promoting gives the group a status glyph and a live line and leaves the children alone. The left end of the drop line sets the depth; drag sideways to change it. A stopped meta-agent stays in place as ✓, and its children move up one level and keep running.

### Screen 8 — Provenance history for one item
```
roundup   payments-api                                                                     inbox 4

  ○ checkout                    │ auth- │ history of #5 session store on redis               close
    planning: split #7 into th… │       │
›   ○ auth-refactor             │ > [fr │                13:00     13:30     14:00     14:30
      editing src/auth/token.ts │   mai │ you               Y
    ○ token-tests        ci red │       │ checkout               C      C
      vitest auth/  42 of 118   │ Read  │ session-store            sS     s  s s s
    ⏸ session-store             │ Updat │
      waits on #4 migrate       │   rot │ time   who            item
  ▾ migrate                     │   { t │ 14:12  session-store  4 reads, 13:50 to 14:12
    ○ rename-cols               │ Bash  │ 13:46  checkout       wrote  + waits on #4
      writing 0042_rename.sql   │   18  │ 13:31  session-store  wrote  claimed it
    · backfill                  │ Updat │ 13:30  session-store  read
      idle 6m                   │ Read  │ 13:24  checkout       wrote  on: session-store
  ● GATEWAY                     │       │ 13:10  you            wrote  created
    asks: keep v1 routes?    4m │       │
  ✕ WEBHOOKS                    │       │
    exited 137, out of memory   │       │
  ○ docs                        │       │
    writing docs/auth.md        │       │
  · perf-probe                  │       │
    idle 22m                    │       │
  ○ npm run dev                 │       │
    ready on :3000              │       │
  ✓ 2 done                      │       │
                                │       │
  + agent  + terminal  + group  │ > █   │
```
Each lane plots every touch (capital = wrote, small = read), and the rightmost mark is the letter shown at the end of the row in the shelf. The log below uses the inbox's grammar (time, who, item) and folds repeats. Click a `wrote` row to see its diff.

### Screen 9 — Extensions (settings window)
```
roundup settings                                                            project   › extensions

extensions   ~/.roundup/extensions   hot-reload on save                                 reload all

               agent           rail          ui     tool  bus    provenance
module         status classify group   sort  render call  route  render
ci-watch       ▪                             ▪                                loaded 14:02
pr-chip                                      ▪                   ▪            loaded 11:20
auto-group            ▪        ▪                                              off
TIDY-SORT                              ✕                                      threw 14:31
  TypeError: a.since is undefined   index.js:14   skipped 37 calls            reload   off
route-guard                                         ▪     ▪                   loaded 09:15

  open folder
```
This is a separate macOS settings window. Rows are modules in `next` order (drag to reorder) and columns are the eight hooks; ✕ marks the hook that threw. Only the failing module carries ink.

### Screen 10 — Project settings (pad storage, delivery routes)
```
roundup settings                                                            › project   extensions

payments-api   ~/repos/payments-api

pads
  storage         in app            store as .md files
                  as files they live in .roundup/pads/<name>.md
  export          all 4 pads as .md

delivery between agents
  default         auto-inject when the receiver next goes idle, tagged [from sender]

  from            to                delivery      set by
  any             any               auto          default
  token-tests     auth-refactor     ask first     you 14:30
  gateway         any               ask first     route-guard
  + route
```
One click on a `delivery` value flips it between auto and ask first, and `set by` records who decided. Messages on an ask-first route land in the inbox as held ● rows.

### Screen 11 — First run, empty
```
roundup

                                │                                       │
  agents and terminals          │                                       │ todos
  appear here, one per row,     │ open a folder to start                │ agents add them as they
  nested by indent              │                                       │ plan; so can you
                                │ ~/repos/payments-api                  │
                                │ ~/repos/web-app                       │ pads
                                │ choose folder…                        │ ◈ agent notes
                                │                                       │ ◇ yours
                                │                                       │
                                │ claude 2.1.4  /opt/homebrew/bin       │
                                │                                       │
  + agent  + terminal  + group  │                                       │
```
Each region is labelled in place with what will live there; there is no tutorial. No terminal exists until the first agent does. If claude isn't installed, that line becomes the only ink in the window: `✕ CLAUDE NOT FOUND, install Claude Code`.


## Decisions the drawings left open (recommendations accepted by "go")

1. A collapsed Group shows its most urgent child's glyph and ink plus a light child count; expanded, only ▾.
2. Done Agents fold into one `✓ n done` line per tree level after 10 minutes; a click unfolds them.
3. Off-screen exceptions get one pinned inked line (`● 1 below`) that scrolls to the row on click. The rail never reorders.
4. Background attention = Dock badge (needs-you + error count) only; no macOS banners.
5. Each Agent gets a unique fixed initial at spawn (first letter of its name not yet taken in the Project). `Y` is reserved for the user; hovering a letter shows the full name.
6. The Inbox excludes auto-delivered agent↔agent traffic. It holds Messages to the user, Held Messages, unblocked Todos and changed Pads.
7. `agent.status` and `agent.classify` may return labels and Chips only, never `kind`. The built-in classifier owns it.
8. A user's Route click wins over any `bus.route` Hook; `set by` shows who decided.
9. A Pad's owner may rewrite it; everyone else may only append. Appends count as writes in Provenance.
10. File-backed Pads live in the Project and are not auto-ignored; flipping back to app-only re-imports.
11. Rows nest only under Groups and Meta-agents; nesting never silently promotes.
12. Worktree-per-agent is a per-Project default, off, with no choice at spawn time.
13. The Shelf is project-wide and does not filter by selection.
