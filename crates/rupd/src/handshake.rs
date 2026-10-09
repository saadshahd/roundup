//! The one line an attached App writes to the Daemon's stdin before anything else (H18): the
//! `proof` that `decision.answer` needs (H4). Nothing here prints or returns the proof in an error.

use std::fmt;
use std::io::{self, BufRead, Read};
use std::time::Duration;

/// How long an attached Daemon waits for the line: the App's own startup bound (S1).
pub const HANDSHAKE_BOUND: Duration = Duration::from_secs(10);

const TAG: &str = "roundup-proof 1 ";
const PROOF_LEN: usize = 64;
/// Tag, proof and newline; a longer line is refused without reading the rest of it.
const LINE_MAX: usize = TAG.len() + PROOF_LEN + 1;

/// The proof the App created for this Daemon; `Debug` hides it.
#[derive(Clone, PartialEq, Eq)]
pub struct Handshake(String);

impl Handshake {
    pub fn into_proof(self) -> String {
        self.0
    }
}

impl fmt::Debug for Handshake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Handshake(..)")
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum HandshakeError {
    /// End of file, or the bound passed, before the newline.
    Incomplete,
    Oversized,
    Malformed,
    Io(io::ErrorKind),
}

impl fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Incomplete => f.write_str("the handshake ended before its newline"),
            Self::Oversized => write!(f, "the handshake is longer than {LINE_MAX} bytes"),
            Self::Malformed => {
                write!(
                    f,
                    "the handshake is not `{TAG}` and {PROOF_LEN} lowercase hex digits"
                )
            }
            Self::Io(kind) => write!(f, "the handshake could not be read ({kind})"),
        }
    }
}

impl std::error::Error for HandshakeError {}

/// Reads exactly one handshake line, leaving every later byte in `input`.
pub fn read_handshake(input: &mut impl BufRead) -> Result<Handshake, HandshakeError> {
    let mut line = Vec::new();
    let read = input
        .by_ref()
        .take(LINE_MAX as u64 + 1)
        .read_until(b'\n', &mut line)
        .map_err(|err| HandshakeError::Io(err.kind()))?;
    if line.last() != Some(&b'\n') {
        return Err(if read > LINE_MAX {
            HandshakeError::Oversized
        } else {
            HandshakeError::Incomplete
        });
    }
    if line.len() > LINE_MAX {
        return Err(HandshakeError::Oversized);
    }
    let text =
        std::str::from_utf8(&line[..line.len() - 1]).map_err(|_| HandshakeError::Malformed)?;
    match text.strip_prefix(TAG) {
        Some(proof)
            if proof.len() == PROOF_LEN
                && proof
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) =>
        {
            Ok(Handshake(proof.to_owned()))
        }
        _ => Err(HandshakeError::Malformed),
    }
}
