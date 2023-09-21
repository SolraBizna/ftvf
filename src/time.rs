#[cfg(not(feature="no_std"))]
mod realtime;
#[cfg(not(feature="no_std"))]
pub use realtime::RealtimeNowSource;

use core::time::Duration;

#[cfg_attr(not(feature="no_std"), doc="\
A source of time information for [`Metronome`](struct.Metronome.html) to use. \
For most purposes, [`RealtimeNowSource`](struct.RealtimeNowSource.html) will \
be sufficient. For non-realtime applications (such as rendering \
pre-determined gameplay footage), see \
[`FakeNowSource`](struct.FakeNowSource.html).")]
#[cfg_attr(feature="no_std", doc="\
A source of time information for [`Metronome`](struct.Metronome.html) to use. \
Because you are using the `no_std` feature, you will need to provide your own \
`NowSource` for realtime use. For non-realtime applications (such as \
rendering pre-determined gameplay footage), see \
[`FakeNowSource`](struct.FakeNowSource.html).")]
pub trait NowSource {
    type Instant: TemporalSample;
    /// Return a point in time representing Now.
    fn now(&mut self) -> Self::Instant;
}

/// A type that represents a particular point in time. You only need to worry
/// about it if you're implementing your own timing routines.
pub trait TemporalSample : Sized + Clone + PartialOrd + PartialEq {
    /// If this TemporalSample is *after* the given origin, return the
    /// `Duration` that has passed since that point. If this TemporalSample is
    /// *before* the given origin, return `None`.
    fn time_since(&self, origin: &Self) -> Option<Duration>;
    /// Return a new TemporalSample that is this much time into the future.
    ///
    /// You must have nanosecond precision. If your underlying type does not 
    /// have nanosecond precision, *you* must keep track of the residual.
    fn advanced_by(&self, amount: Duration) -> Self;
    /// As `advanced_by`, but mutates self instead of returning a new value.
    /// The default implementation just does
    /// `*self = self.advanced_by(amount)`.
    fn advance_by(&mut self, amount: Duration) {
        *self = self.advanced_by(amount);
    }
}

