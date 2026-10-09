---
name: to-spec
description: Turn an observed gap into a bounded scenario and a GitHub work Issue.
---

Read `.agents/data/work.md`. Search existing Issues, PRs and scenario headings first. Reuse an owner when one covers the gap.

Use the current conversation and accepted product/design documents to specify the outcome, ownership, observer and regressions. Keep acceptance in the scenario; publish the work order as an Issue with native blocking dependencies and an immutable Key. Add `ready-for-agent` only when scope and dependencies are settled. Never create a scenario Work table. A consequential product choice absent from the accepted documents stays a question on the Issue; ordinary engineering choices do not go to the user.
