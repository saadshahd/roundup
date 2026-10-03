//! The wire contract: every RPC parameter, result and event, as Rust types.
//! `cargo test -p contracts` regenerates `contracts/generated/*.ts`; CI fails if that changes the tree.
//! Owned by the Architect. Editing this crate is a Contract change (AGENTS.md rule 4).

pub mod agent;
pub mod common;
pub mod decision;
pub mod event;
pub mod methods;
pub mod pad;
pub mod terminal;
pub mod todo;

pub use common::*;
pub use event::{Event, EventData};
