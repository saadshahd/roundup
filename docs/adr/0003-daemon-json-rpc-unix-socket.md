# rupd daemon over a Unix socket, JSON-RPC 2.0

Status: Accepted

## Decision

Core runs as `rupd`; UI, `rup` CLI, MCP server and Extensions are clients over a local Unix socket speaking JSON-RPC 2.0.

## Considered

In-process Tauri commands only, gRPC, HTTP on localhost.

## Why

One seam serves every client, so the UI has no privileged access and Extensions get the same surface. A Unix socket is local-only by file permissions; JSON-RPC is typed, tiny and has notifications for events.

## Notes

Contract lives in `contracts/`. Any edit is a Contract change (rule 4).
