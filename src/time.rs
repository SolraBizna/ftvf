#[cfg(not(feature="no_std"))]
mod realtime;
#[cfg(not(feature="no_std"))]
pub use realtime::RealtimeNowSource;

use core::time::Duration;

/// A source of time information for [`Metronome`](struct.Metronome.html) to
/// use. For most purposes,
/// [`RealtimeNowSource`](struct.RealtimeNowSource.html) will be sufficient.
pub trait NowSource : Copy {
    type Instant: TemporalSample + Clone + PartialOrd + PartialEq;
    /// Return a point in time representing Now.
    fn now(&mut self) -> Self::Instant;
    /// Sleep until at least `how_long` from *now*. Optional.
    ///
    /// Will only be called very soon after `now()`. Any attempt to account for
    /// temporal slippage would do more harm than good.
    #[allow(unused)]
    fn sleep(&mut self, how_long: Duration) {}
}

/// A type that represents a particular point in time. You only need to worry
/// about it if you're implementing your own timing routines.
pub trait TemporalSample : Sized {
    /// If this TemporalSample is *after* the given origin, return the
    /// `Duration` that has passed since that point. If this TemporalSample is
    /// *before* the given origin, return `None`.
    fn time_since(&self, origin: &Self) -> Option<Duration>;
    /// Return a new TemporalSample that is this far in the future.
    ///
    /// If you cannot advance precisely (because your timebase is less precise
    /// than `Duration`), you must always undershoot, never overshoot. In
    /// addition, *you* must keep track of the residual, and apply it to the
    /// next call(s) to `advance`, so that, *over time*, the missing time
    /// eventually gets caught up with.
    fn advanced_by(&self, amount: Duration) -> Self;
    /// As `advanced_by`, but mutates self instead of returning a new value.
    /// The default just does `*self = self.advanced_by(amount)`.
    fn advance_by(&mut self, amount: Duration) {
        *self = self.advanced_by(amount);
    }
}

