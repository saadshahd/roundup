# Triage Labels

The skills speak in terms of five canonical triage roles. roundup's loop has no triage stage, so four roles map to no label (`.agents/data/work.md`).

| Label in mattpocock/skills | Label in our tracker | Meaning                                                                                               |
| -------------------------- | -------------------- | ----------------------------------------------------------------------------------------------------- |
| `needs-triage`             | _none_               | An open Issue the user filed is already queued; one with no `Key:` line runs as `Mode: specify` (L34) |
| `needs-info`               | _none_               | Ask in an Issue comment; only the askers L79 lists apply `flag:needs-user`                            |
| `ready-for-agent`          | _none_               | An open Issue the user filed is already queued (L34)                                                  |
| `ready-for-human`          | _none_               | A Builder run takes every Issue and asks the user itself on a product question (L79)                  |
| `wontfix`                  | `wontfix`            | Will not be actioned; close the Issue as not planned                                                  |

When a skill mentions a role (e.g. "apply the AFK-ready triage label"), use the corresponding label string from this table; for _none_, apply no label.
