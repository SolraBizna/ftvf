use core::{
    ops::AddAssign,
    time::Duration,
};

use super::{NowSource, TemporalSample};

/// A fake `NowSource` that is entirely under your control. Thinly wraps a
/// `Duration` representing the current "now", starting at zero. You can
/// manipulate time by manipulating the `now` field directly, or by using `+=`
/// with a `Duration` on the right hand side.
#[derive(Debug, Default, Copy, Clone)]
pub struct FakeNowSource {
    /// How long since an arbitrary origin. Manipulate this field directly to
    /// manipulate time itself. (Don't kill your own grandfather.)
    pub now: Duration,
}

impl NowSource for FakeNowSource {
    type Instant = Duration;
    fn now(&mut self) -> Duration {
        self.now
    }
}

impl AddAssign<Duration> for FakeNowSource {
    fn add_assign(&mut self, rhs: Duration) {
        self.now += rhs
    }
}

impl TemporalSample for Duration {
    fn time_since(&self, origin: &Duration) -> Option<Duration> {
        self.checked_sub(*origin)
    }
    fn advanced_by(&self, amount: Duration) -> Duration {
        *self + amount
    }
}
