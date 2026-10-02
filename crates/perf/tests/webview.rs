//! Scenario R11 (`scenarios/perf.md`): what the keystroke runner accepts from the webview.

use perf::webview::{checked, perf_line};
use serde_json::json;

fn report(frame_ms: f64, dropped: f64) -> serde_json::Value {
    json!({ "p95": 9.0, "frame_ms": frame_ms, "dropped": dropped, "keys": 1000 })
}

#[test]
fn r11_the_perf_line_is_found_once_complete_among_echoed_keys() {
    assert_eq!(perf_line("aaaPERF {\"p95\":9}"), None);
    assert_eq!(perf_line("aaaPERF {\"p95\":9}\nrest"), Some("{\"p95\":9}"));
}

#[test]
fn r11_a_full_rate_run_with_few_dropped_keys_is_accepted() {
    assert!(checked(&report(16.7, 50.0)).is_ok());
}

#[test]
fn r11_a_throttled_window_is_refused_naming_the_frame_time() {
    let err = checked(&report(500.0, 0.0)).unwrap_err().to_string();

    assert!(err.contains("500.0 ms") && err.contains("front"));
}

#[test]
fn r11_more_than_five_percent_dropped_keys_is_refused() {
    assert!(checked(&report(16.7, 51.0)).is_err());
}

#[test]
fn r11_a_failure_the_webview_reported_is_refused_with_its_message() {
    let err = checked(&json!({ "error": "the + terminal button did not appear" }))
        .unwrap_err()
        .to_string();

    assert!(err.contains("+ terminal button"));
}
