# SQLite (WAL) for state; Pads as markdown

Status: Accepted

## Decision

`rusqlite` in WAL mode for Todos, Pad index, Touch log, Routes, Agent tree. Pads app-only by default; per-Project setting stores them as `.md` files in `.roundup/pads/`.

## Considered

Plain JSON files, sled, Postgres.

## Why

Blocker graph and append-only Touch log want transactions and queries. Markdown files keep Pads git-friendly when the user opts in.

## Notes

Terminology: the table and RPC names say Pad, never scratchpad or session (rule 6).
