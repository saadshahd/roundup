Builder. `prime to load the taste rules, tdd to land H11, H12 and H16 red then green`

Scenarios H11, H12, H16 in `scenarios/control.md`; owns and waits as its Work row says. Also read `docs/control-channel.md` and `spikes/hooks-permission/REPORT.md` (real strings and timings); tests use the fake `claude` and an injected clock.

- One write path `Agents::prompt(id, text) -> Result<(), PromptError>` (`Busy`, `NotAccepted`, `NotFound`), `pub`, no RPC types in its signature (the B-series Mailbox calls it in-process); `agent.prompt {id, text}` sits over it.
- H12 deletes `SUBMIT_DELAY`; a test with a clock that never advances still delivers.
- Error codes in `crates/rpc/src/error.rs`: `BUSY = -32005`, `NOT_ACCEPTED = -32006`, `NOT_RUNNING = -32007`, `NOT_ACKED = -32008`, registered wherever `NOT_FOUND` is, each round-tripped through `RpcError` in a test.
