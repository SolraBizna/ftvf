use core::{
    num::NonZeroU32,
    time::Duration,
};

/// A frequency, measured by a ratio of seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rate {
    /// Ticks.
    pub(crate) numerator: NonZeroU32,
    /// Seconds.
    pub(crate) denominator: NonZeroU32,
    pub(crate) duration_per: Duration,
    /// The fraction of a nanosecond that accumulates each tick. The numerator
    /// is `residual_per` and the *denominator* is `numerator`.
    pub(crate) residual_per: u32,
}

impl Rate {
    /// Creates a new Rate with the given numerator and denominator. The
    /// denominator is seconds.
    ///
    /// PANICS if the numerator or denominator are zero, or are greater than
    /// one billion!
    pub fn per_second(numerator: u32, denominator: u32) -> Rate {
        assert_ne!(numerator, 0, "The numerator and denominator cannot be zero.");
        assert_ne!(denominator, 0, "The numerator and denominator cannot be zero.");
        assert!(numerator <= 1_000_000_000, "The numerator and denominator may not exceed 1,000,000,000.");
        assert!(denominator <= 1_000_000_000, "The numerator and denominator may not exceed 1,000,000,000.");
        Self::per_second_nonzero(NonZeroU32::new(numerator).unwrap(), NonZeroU32::new(denominator).unwrap())
    }
    /// Creates a new Rate with the given numerator and denominator. The
    /// denominator is seconds.
    ///
    /// YOU must ensure that the numerator and denominator do not exceed one
    /// billion.
    pub const fn per_second_nonzero(numerator: NonZeroU32, denominator: NonZeroU32) -> Rate {
        let numerator_int = numerator.get();
        let denominator_int = denominator.get();
        // numerator/denominator = ticks/second
        // 1G*denominator/numerator = nanoseconds/tick
        let denominator_in_nanoseconds = 1_000_000_000u64 * denominator_int as u64;
        let number_of_nanoseconds
            = denominator_in_nanoseconds / (numerator_int as u64);
        let residual
            = denominator_in_nanoseconds % (numerator_int as u64);
        debug_assert!(residual < u32::MAX as u64);
        Rate {
            numerator: numerator,
            denominator: denominator,
            duration_per: Duration::from_nanos(number_of_nanoseconds),
            residual_per: residual as u32,
        }
    }
}

