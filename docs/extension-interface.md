# Extension interface (v0)

Deliberately tiny. An Extension is one module exporting `register(on)`, named by one path in its manifest. Every Hook has one shape: `($, e, next)`.

## Manifest

`rup-extension.json` = `{ name, module, permissions, types? }`. Permissions are declared and enforced by `rupd`. Panels are sandboxed iframe `Client` components mounted by `ui.render`.

## Shape

- `$` — typed engine handle: `$.agents`, `$.todos`, `$.pads`, `$.bus`, `$.ui`, `$.store`, `$.clock`.
- `e` — the event as plain data.
- `next(e)` passes on. `next({...e, changed})` rewrites what later Hooks see. Returning a value without `next` answers for yourself.
- Results are small discriminated unions: `{deny}`, `{status}`, `{group}`.
- Registration: `on(event, optionalMatcher, hook)`. Matchers replace if-chains: `{agent:{role:"reviewer"}}`, `{component:"Rail"}`.
- Hot reload on save: timers dropped, `register` re-runs. State lives in `$.store` (persisted, per-Agent key), so reload is safe.
- A throwing Hook is skipped (fail open) with one toast. `.catch` can answer in its place.
- Pure core, thin glue: decisions are exported pure functions. A mock `$` harness ships in the SDK (`rup ext test`).

```ts
export function register(on) {
  on("agent.status", { agent: { role: "reviewer" } }, ($, e, next) =>
    e.checks?.failed ? { status: [{ label: "ci red", tone: "dim" }] } : next(e));
}
```

## Hooks (each has a built-in default; the default is itself a module)

| Concern | Hook | Returns |
|---|---|---|
| Group Agents | `rail.group` (e: agent) | `{group: path[]}` — nesting is a path, so reorder/nest/unnest = edit path + `order` |
| Status labels | `agent.status` | `{label, tone}[]` chips. **Never `kind`** — the built-in classifier owns it |
| Categorise | `agent.classify` | `{tags: string[]}` |
| Sort / order | `rail.sort` | comparator key; user drag-reorder persists as `order` in `$.store` and wins |
| Render | `ui.render {component}` | element tree (`Rail`, `Row`, `Shelf`, `Inbox`, `Statusbar`) |
| Gate tools | `tool.call` | `{deny}` / `{context}` |
| Delivery between Agents | `bus.route` | `{deliver: auto\|ask-first\|drop}`; a user click wins |
| Provenance chips | `provenance.render` | chips folded from the Touch log (default: last toucher letter) |

Note on `agent.status`: the glyph is the most urgent Kind. Hooks add labels and Chips only (decision 7 in `wireframes.md`).

## Provenance

`rupd` records every RPC and MCP call with `actor` (`{kind:"user"|"agent"|"ext", id, parent}`) in the append-only Touch log. Todos and Pads expose `readBy[]` / `touchedBy[]`, derived from the log. Events carry `actor`. Queries: `$.todos.history(id)`, `$.agents.touched(agentId)`.

## Bus

Typed Messages `{from: Actor, to: Actor|Group|Topic, kind, body, replyTo}`. Subscriptions are by event (`todo.unblocked`, `agent.idle`, `pad.changed`), never by clock. Hooks never start a turn; delivery is a queued prompt the Agent consumes at its next safe point. Anything addressed to the user pre-fills and never submits.

## Meta-agents

An Agent node has `parent`. A Meta-agent is an Agent whose children are Agents; it receives child events and can message them. Users drag to reorder (writes `parent` + `order`); Extensions use `$.agents.move(id, {parent, index})`. `rail.group` proposes, manual placement wins.

## Out of scope for v0

Other vendors' adapters (`agentAdapters` stays the seam), a registry, network permissions beyond declared hosts.
