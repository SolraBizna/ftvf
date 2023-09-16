use core::time::Duration;

use super::{Rate, TemporalSample};

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub(crate) struct PreciseInstant<Instant: TemporalSample> {
    pub(crate) at: Instant,
    residual: u32,
}

impl<Instant: TemporalSample> From<Instant> for PreciseInstant<Instant> {
    fn from(at: Instant) -> Self {
        Self { at, residual: 0 }
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
    pub(crate) fn forget_residual(&mut self) {
        self.residual = 0;
    }
}