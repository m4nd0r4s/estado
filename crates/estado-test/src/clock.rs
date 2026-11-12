//! Readable durations for tests: `13.secs()`, `250.millis()`.

use core::time::Duration;

/// Builds a [`Duration`] from an integer.
///
/// ```
/// use estado_test::DurationExt;
/// assert_eq!(2.secs() + 500.millis(), std::time::Duration::from_millis(2500));
/// ```
pub trait DurationExt {
    /// That many seconds.
    fn secs(self) -> Duration;
    /// That many milliseconds.
    fn millis(self) -> Duration;
}

impl DurationExt for u64 {
    fn secs(self) -> Duration {
        Duration::from_secs(self)
    }

    fn millis(self) -> Duration {
        Duration::from_millis(self)
    }
}
