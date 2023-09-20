use core::{
    num::NonZeroU32,
    time::Duration,
};

/// A frequency, measured by a ratio of seconds.
#[derive(Debug, Clone, Copy)]
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

impl PartialEq for Rate {
    fn eq(&self, other: &Self) -> bool {
        self.numerator == other.numerator && self.denominator == other.denominator
    }
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
        let gcd = gcd(numerator_int, denominator_int);
        let (numerator_int, denominator_int) = (numerator_int / gcd, denominator_int / gcd);
        // numerator/denominator = ticks/second
        // 1G*denominator/numerator = nanoseconds/tick
        let denominator_in_nanoseconds = 1_000_000_000u64 * denominator_int as u64;
        let number_of_nanoseconds
            = denominator_in_nanoseconds / (numerator_int as u64);
        let residual
            = denominator_in_nanoseconds % (numerator_int as u64);
        debug_assert!(residual < u32::MAX as u64);
        Rate {
            numerator: unsafe { NonZeroU32::new_unchecked(numerator_int) },
            denominator: unsafe { NonZeroU32::new_unchecked(denominator_int) },
            duration_per: Duration::from_nanos(number_of_nanoseconds),
            residual_per: residual as u32,
        }
    }
}

const fn gcd(a: u32, b: u32) -> u32 {
    let (mut big, mut small) = if a > b { (a,b) } else { (b,a) };
    while big != small {
        big = big % small;
        if big == 0 { return small }
        else { (small, big) = (big, small) }
    }
    big
}

#[cfg(test)]
mod test {
    use super::*;
    #[cfg(feature="no_std")]
    use std::prelude::*;
    #[test]
    fn gcdtest() {
        let test_set = [
            (60000, 1001, 1),
            (64, 288, 32),
            (114411, 258522, 33),
        ];
        for (a, b, answer) in test_set.into_iter() {
            assert_eq!(gcd(a,b), answer);
        }
    }
}