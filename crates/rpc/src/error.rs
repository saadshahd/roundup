use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 error codes, plus roundup's own in the server-defined range.
pub mod code {
    pub const PARSE_ERROR: i64 = -32700;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL: i64 = -32603;
    pub const NOT_FOUND: i64 = -32001;
    /// The caller is not allowed to do this (for example, rewriting a Pad it does not own).
    pub const FORBIDDEN: i64 = -32002;
    /// The request is well-formed but conflicts with current state (for example, a blocker cycle).
    pub const CONFLICT: i64 = -32003;
    /// The connection closed after the request line was written but before a reply arrived: the
    /// Daemon may or may not have run the call.
    pub const UNKNOWN_OUTCOME: i64 = -32004;
    /// The Agent is not `idle`, so a Steer would land in the middle of its turn (H11).
    pub const BUSY: i64 = -32005;
    /// A Steer was written and its `UserPromptSubmit` Signal never came (H11).
    pub const NOT_ACCEPTED: i64 = -32006;
    /// The Agent has no turn to Interrupt (H13).
    pub const NOT_RUNNING: i64 = -32007;
    /// An Interrupt was written and the title never changed (H13).
    pub const NOT_ACKED: i64 = -32008;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{message} (code {code})")]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(
            code::METHOD_NOT_FOUND,
            format!("method not found: {method}"),
        )
    }

    pub fn not_found(what: impl std::fmt::Display) -> Self {
        Self::new(code::NOT_FOUND, format!("not found: {what}"))
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(code::FORBIDDEN, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(code::CONFLICT, message)
    }

    pub fn busy(message: impl Into<String>) -> Self {
        Self::new(code::BUSY, message)
    }

    pub fn not_accepted(message: impl Into<String>) -> Self {
        Self::new(code::NOT_ACCEPTED, message)
    }

    pub fn not_running(message: impl Into<String>) -> Self {
        Self::new(code::NOT_RUNNING, message)
    }

    pub fn not_acked(message: impl Into<String>) -> Self {
        Self::new(code::NOT_ACKED, message)
    }

    /// For failures that are the Daemon's fault, never the caller's.
    pub fn internal(err: impl std::fmt::Display) -> Self {
        Self::new(code::INTERNAL, err.to_string())
    }

    /// The connection dropped before the reply: the call may have run.
    pub fn unknown_outcome(message: impl Into<String>) -> Self {
        Self::new(code::UNKNOWN_OUTCOME, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn h11_the_steer_and_interrupt_codes_round_trip_through_rpc_error() {
        let codes = [
            (code::BUSY, -32005),
            (code::NOT_ACCEPTED, -32006),
            (code::NOT_RUNNING, -32007),
            (code::NOT_ACKED, -32008),
        ];
        for (code, wire) in codes {
            assert_eq!(code, wire);
            let error = RpcError::new(code, "agent 7");
            let json = serde_json::to_string(&error).unwrap();
            assert_eq!(serde_json::from_str::<RpcError>(&json).unwrap(), error);
        }
        assert_eq!(RpcError::busy("x").code, code::BUSY);
        assert_eq!(RpcError::not_accepted("x").code, code::NOT_ACCEPTED);
    }
}
