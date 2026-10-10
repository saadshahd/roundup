# Triage Labels

The skills speak in terms of five canonical triage roles. This file maps those roles to the labels roundup's loop reads (`.agents/data/work.md`).

| Label in mattpocock/skills | Label in our tracker | Meaning                                                                      |
| -------------------------- | -------------------- | ---------------------------------------------------------------------------- |
| `needs-triage`             | `loop:work`          | Queued; a Builder run with `Mode: specify` triages it                        |
| `needs-info`               | `flag:needs-user`    | Waiting on the user; holds the Issue out of the queue (L79)                  |
| `ready-for-agent`          | `loop:work`          | Queued; fully specified, so a Builder run implements it                      |
| `ready-for-human`          | `flag:needs-user`    | Needs the user's decision; removing the label returns the Issue to the queue |
| `wontfix`                  | `wontfix`            | Will not be actioned; close the Issue as not planned                         |

When a skill mentions a role (e.g. "apply the AFK-ready triage label"), use the corresponding label string from this table.
