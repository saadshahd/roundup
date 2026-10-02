use std::io;

use serde_json::Value;

/// Frames slower than this mean the window was not drawn at full rate (behind other windows, occluded): the run says nothing about rendering.
const FRAME_LIMIT_MS: f64 = 25.0;
const MAX_DROPPED_SHARE: f64 = 0.05;

pub fn failure(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

/// The `PERF <json>` line in what a Terminal printed, if it is complete.
pub fn perf_line(printed: &str) -> Option<&str> {
    let start = printed.find("PERF ")?;
    let line = &printed[start + "PERF ".len()..];

    line.find('\n').map(|end| &line[..end])
}

/// A report that proves nothing is an error, not a number: the webview's own failure, a throttled window, or too many dropped keys.
pub fn checked(report: &Value) -> io::Result<()> {
    if let Some(error) = report["error"].as_str() {
        return Err(failure(format!("the webview failed: {error}")));
    }

    let number = |name: &str| {
        report[name]
            .as_f64()
            .ok_or_else(|| failure(format!("the report has no {name}")))
    };
    let frame = number("frame_ms")?;
    let dropped = number("dropped")?;
    let keys = number("keys")?;

    if frame > FRAME_LIMIT_MS {
        return Err(failure(format!(
            "frames took {frame:.1} ms on average: the window was not drawn at full rate; bring it to the front and rerun"
        )));
    }

    if dropped > keys * MAX_DROPPED_SHARE {
        return Err(failure(format!("{dropped} of {keys} keys were dropped")));
    }

    Ok(())
}
