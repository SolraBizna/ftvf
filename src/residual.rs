use core::{
    cmp::{PartialEq,PartialOrd},
    convert::TryInto,
    time::Duration,
};

use super::{Rate, TemporalSample};

/// This is an internal type that aids in tracking the residual
/// (sub-nanosecond) element of a timestamp. The caller must keep track of the
/// denominator of the residual and call `forget_residual` any time it changes.
#[derive(Debug, Clone)]
pub(crate) struct PreciseInstant<Instant: TemporalSample> {
    pub(crate) at: Instant,
    pub(crate) residual: u32,
}

impl<Instant: TemporalSample> From<Instant> for PreciseInstant<Instant> {
    fn from(at: Instant) -> Self {
        Self { at, residual: 0 }
    }
}

impl<Instant: TemporalSample> PartialEq for PreciseInstant<Instant> {
    fn eq(&self, other: &Self) -> bool {
        self.at == other.at
    }
}

impl<Instant: TemporalSample> PartialOrd for PreciseInstant<Instant> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.at.partial_cmp(&other.at)
    }
}

impl<Instant: TemporalSample> PreciseInstant<Instant> {
    pub(crate) fn next(&self, rate: &Rate) -> Self {
        let at = self.at.advanced_by(rate.duration_per);
        let residual = self.residual + rate.residual_per;
        if residual >= rate.numerator.get() {
            let residual = residual - rate.numerator.get();
            debug_assert!(residual < rate.numerator.get());
            Self { at: at.advanced_by(Duration::from_nanos(1)), residual }
        } else { Self { at, residual } }
    }
    pub(crate) fn nth(&self, n: u32, rate: &Rate) -> Self {
        let at = self.at.advanced_by(rate.duration_per * n);
        let residual = self.residual as u64 + rate.residual_per as u64 * n as u64;
        if residual >= rate.numerator.get() as u64 {
            let advance_by = residual / rate.numerator.get() as u64;
            let residual = residual % rate.numerator.get() as u64;
            Self { at: at.advanced_by(Duration::from_nanos(advance_by)), residual: residual as u32 }
        } else { Self { at, residual: residual as u32 } }
    }
    // approximate!
    pub(crate) fn ticks_until(&self, target_time: &Instant, rate: &Rate) -> u32 {
        let difference = match target_time.time_since(&self.at) {
            None => return 0,
            Some(x) => x,
        };
        (difference.as_nanos() / rate.duration_per.as_nanos()).try_into()
            .unwrap_or(u32::MAX)
    }
    pub(crate) fn last_tick_before(&self, target_time: &Instant, rate: &Rate) -> PreciseInstant<Instant> {
        let surplus = self.ticks_until(target_time, rate);
        self.nth(surplus, rate)
    }
    pub(crate) fn forget_residual(&mut self) {
        self.residual = 0;
    }
}