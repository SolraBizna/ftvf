use std::time::{Duration, Instant};

use crate::{TemporalSample, NowSource};

/// A [`NowSource`](trait.NowSource.html) that uses the standard Rust timing
/// facilities to obtain its timing information. This is the default
/// `NowSource`, and also the one you almost certainly want to use.
#[derive(Debug,Copy,Clone)]
pub struct RealtimeNowSource {}

impl RealtimeNowSource {
    pub fn new() -> RealtimeNowSource { RealtimeNowSource { } }
}

impl NowSource for RealtimeNowSource {
    type Instant = std::time::Instant;
    fn now(&mut self) -> Self::Instant { Self::Instant::now() }
    fn sleep(&mut self, how_long: Duration) { std::thread::sleep(how_long) }
}

impl TemporalSample for Instant {
    fn time_since(&self, origin: &Self) -> Option<Duration> {
        self.checked_duration_since(*origin)
    }
    fn advanced_by(&self, amount: Duration) -> Self {
        *self + amount
    }
    fn advance_by(&mut self, amount: Duration) {
        *self += amount;
    }
}

#[cfg(test)]
mod test {
    use super::*;
    // I *think* `Instant` is guaranteed to be able to store time with as much
    // precision as `Duration` on any platform. If it's not...
    #[test] fn instant_residual() {
        let base = Instant::now();
        for n in 1 .. 100 {
            let duration = Duration::new(0, n);
            let plussed = base + duration;
            let difference = plussed - base;
            assert_eq!(difference, duration,
                "Instant is not as precise as Duration on your platform. \
                 Please email Solra Bizna <solra@bizna.name> about this. \
                 Include in your email what platform you are running on.");
        }
    }
}