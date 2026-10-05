//! Scenario R15 (`scenarios/perf.md`): `terminal_write_p95_ms` paces its writes.

use std::cell::Cell;
use std::io;
use std::time::Duration;

use perf::measure::paced_writes;

/// The Terminal's refusal: `terminal.write` fails once this many writes sit unread.
const CAP: usize = 16;

#[tokio::test]
async fn r15_a_thousand_writes_never_leave_more_than_one_unread_and_time_only_the_write() {
    let unread = Cell::new(0);
    let most = Cell::new(0);

    let taken = paced_writes(
        async || {
            if unread.get() >= CAP {
                return Err(io::Error::other("terminal 11 is not reading its input"));
            }
            unread.set(unread.get() + 1);
            most.set(most.get().max(unread.get()));
            Ok(())
        },
        async || {
            tokio::time::sleep(Duration::from_millis(1)).await;
            unread.set(unread.get() - 1);
            Ok(())
        },
        1000,
    )
    .await
    .unwrap();

    assert_eq!(taken.len(), 1000);
    assert_eq!(most.get(), 1);
    assert!(
        taken.iter().all(|&ms| ms < 1.0),
        "a write's time includes its 1 ms echo: max {}",
        taken.iter().copied().fold(0.0, f64::max)
    );
}
