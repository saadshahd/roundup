//! One test binary for scenarios/terminal.md (one binary so shared helpers are never dead code).
//! Each module groups the tests of related scenario ids.

mod common;
mod control;
mod in_process;
mod latency;
mod snapshot;
mod spawn;
mod stuck;
mod title;
