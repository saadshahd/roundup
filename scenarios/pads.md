# Pads

**P1 create.** Given an empty Project, when an Agent `a` calls `pad.create {name: "notes", text}`, then the Pad's owner is `a`; creating `notes` again fails with `CONFLICT`, and so does `Notes` (names are unique ignoring case, because the macOS file system is case-insensitive and file-backed Pads would otherwise share one file); `pad.changed {name}` is emitted and a `wrote` Touch on `pad:notes` is logged.

**P2 owner rewrites, others append.** Given Pad `notes` owned by `a`, when `a` calls `pad.write`, then the text is replaced. When another Actor `b` calls `pad.write`, then it fails with `FORBIDDEN` and nothing changes. When `b` calls `pad.append`, then the text is added at the end, `pad.changed` is emitted and a `wrote` Touch by `b` is logged.

**P3 read and list.** Given three Pads, when `pad.list` is called, then all three come back ordered by name; `pad.read {name}` returns one and logs a `read` Touch; an unknown name is `NOT_FOUND`.

**P4 flip ownership.** Given Pad `notes` owned by Agent `a`, when the user calls `pad.setOwner {owner: user}`, then the Pad is the user's (◇) and `a` can no longer rewrite it. `a` may hand it over too. Another Agent `b` calling `pad.setOwner` fails with `FORBIDDEN`.

**P5 file-backed storage.** Given Pads `notes` and `plan`, when `pad.setStorage {files: true}` is called, then `.roundup/pads/notes.md` and `plan.md` exist with the Pads' text, and later writes and appends update the files. When a file is edited on disk and `pad.setStorage {files: false}` is called, then the edit is imported into the Pad. Files are not added to any ignore list. In file mode the app database stays the source of truth and the files are a mirror: `pad.read` returns the database text, and edits made on disk are imported only when flipping back to app storage. Every file write is atomic (temp file then rename) and happens before the database change is committed, so a failure leaves nothing applied; `setStorage` records the new setting only after every file operation succeeded, so a retry redoes the work. A name the file system refuses is `INVALID_PARAMS`.

**P6 export.** Given a Pad, when `pad.export {name, path}` is called, then one `.md` file with the text is written to `path`, whatever the storage setting; a relative `path` is `INVALID_PARAMS` (the Daemon's working directory is not the caller's), and a path whose directory does not exist is an error, never a silent create.

**P7 persistence.** Given app-stored Pads, when `Pads::open` is called again on the same directory, then every Pad, its owner and text are intact.

**P8 names.** A Pad name containing `/`, `\`, `..`, a leading dot, or an empty string fails with `INVALID_PARAMS` for every method that takes a name.

**P9 delete.** Given Pad `notes` owned by `a`, when `a` or the user calls `pad.delete {name}`, then the Pad is gone (`pad.read` is `NOT_FOUND`), its file is removed when file-backed, `pad.changed {name}` is emitted and a `wrote` Touch on `pad:notes` is logged; any other Actor gets `FORBIDDEN`; the name is validated like every other (P8).

**P10 a rejected call leaves no trace, in either storage mode.** Given Pad `notes` owned by Agent `a` in app storage and again in file storage (P5), when another Actor's `pad.write` fails with `FORBIDDEN`, `pad.create` of `Notes` fails with `CONFLICT` (P1), and `pad.read`, `pad.append` and `pad.delete` of `../x` fail with `INVALID_PARAMS` (P8), then after each call `pad.read {name: "notes"}` returns the same text and owner, `.roundup/pads` holds exactly the files it held (none in app storage, no temp file left in file storage), no `pad.changed` arrived and no Touch was logged. A refused call to a Pad that is not there is `NOT_FOUND` and creates no file. Owns `crates/pads/**`. It keeps P1, P2, P5, P8 and P9 green and narrows none.
