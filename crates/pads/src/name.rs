//! Pad names become file names, so they must not be able to escape the pads directory.

use rpc::RpcError;

pub fn validate(name: &str) -> Result<(), RpcError> {
    let bad = name.is_empty()
        || name.starts_with('.')
        || name.contains(['/', '\\'])
        || name.contains("..");
    if bad {
        return Err(RpcError::new(
            rpc::code::INVALID_PARAMS,
            format!("invalid pad name: {name:?}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate;

    #[test]
    fn p8_rejects_separators_dots_and_empty() {
        for name in ["", "a/b", "a\\b", "..", ".hidden", "a..b", "/abs"] {
            assert!(validate(name).is_err(), "{name:?}");
        }
    }

    #[test]
    fn p8_accepts_plain_names() {
        for name in ["notes", "plan-2", "a.b", "Ünï code"] {
            assert!(validate(name).is_ok(), "{name:?}");
        }
    }
}
