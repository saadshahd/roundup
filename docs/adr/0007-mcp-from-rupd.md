# rupd exposes Todos and Pads as an MCP server

Status: Accepted

## Decision

`rupd` serves MCP (stdio shim) so Claude Code Agents create and read Todos and Pads; every call is logged as a Touch with the calling Agent as actor.

## Considered

A bespoke CLI the Agent shells out to.

## Why

MCP is how Claude Code already discovers tools; Agents share state without roundup patching Claude Code.

## Notes

Only Todos and Pads in the MVP.
