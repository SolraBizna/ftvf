//! `ftvf` is a crate for carrying out game logic the One True Way: Fixed
//! Tickrate, Variable Framerate. By having your game logic in strictly
//! fungible ticks, rather than having it vary based on framerate, you gain
//! many advantages:
//!
//! - **Repeatability**: the same inputs will have the same outputs, period.
//! - **Framerate independence**: no issues like Quake had where your exact
//! jump height depends on how fast your computer is.
//! - **Satisfaction**: knowing that you made the morally correct choice. :)
//!
//! To get started, add `ftvf` to your dependencies in `Cargo.toml`:
//!
//! ```toml
//! ftvf = "0.6"
//! ```
//! 
//! then initialize yourself a [`Metronome`](struct.Metronome.html):
//!
//! ```rust
//! # use ftvf::*;
//! # #[cfg(not(feature="no_std"))] {
//! let mut metronome = Metronome::new(
//!   RealtimeNowSource::new(),
//!   // want 30 ticks per 1 second
//!   Rate::per_second(30, 1),
//!   // accept being up to 5 ticks behind
//!   5,
//! );
//! # }
//! ```
//!
//! And then your game loop looks like this:
//!
//! ```rust
//! # use ftvf::*;
//! # #[cfg(not(feature="no_std"))] {
//! # struct GameWorld {}
//! # impl GameWorld {
//! #   fn handle_input(&mut self) {}
//! #   fn perform_tick(&mut self) {}
//! #   fn render(&mut self, _: f32) {}
//! #   fn should_quit(&mut self) -> bool { true }
//! # }
//! # let mut metronome = Metronome::new(RealtimeNowSource::new(), Rate::per_second(30, 1), 5);
//! # let mut world = GameWorld{};
//! while !world.should_quit() {
//!   world.handle_input();
//!   for reading in metronome.sample(Mode::UnlimitedFrames) {
//!     match reading {
//!       Reading::Tick => world.perform_tick(),
//!       Reading::Frame{phase} => world.render(phase),
//!       Reading::TimeWentBackwards
//!         => eprintln!("Warning: time flowed backwards!"),
//!       Reading::TicksLost
//!         => eprintln!("Warning: we're too slow, lost some ticks!"),
//!       // Mode::UnlimitedFrames never returns Idle, but other modes can, and
//!       // this is one way to handle it.
//!       Reading::Idle{duration} => std::thread::sleep(duration),
//!     }
//!   }
//! }
//! # }
//! ```
//!
//! Your logic ticks operate in discrete, fixed time intervals. Then, when it
//! comes time to render, you render a frame which represents time some portion
//! of the way between two ticks, represented by its `phase`. Your rendering
//! process should render an interpolated state between the previous tick and
//! the current tick, based on the value of `phase`. Simple example:
//!
//! ```rust
//! # struct Thingy { previous_position: f32, current_position: f32 }
//! # impl Thingy {
//! #   fn render(&self, phase: f32) {
//! self.render_at(self.previous_position
//!                + (self.current_position - self.previous_position) * phase);
//! #   }
//! #   fn render_at(&self, _: f32) {}
//! # }
//! ```
//!
//! # Changes
//!
//! ## Since 0.5.0
//!
//! - `ftvf` no longer depends on `std`. You can use the `no_std` feature flag
//!   to make the `std` dependency go away, at the cost of not being able to
//!   use the built-in `RealtimeNowSource`.
//! - `Mode::MaxOneFramePerTick` has been renamed to `Mode::OneFramePerTick`.
//! - `metronome.sample()` now returns an iterator directly, instead of making
//!   you repeatedly call `metronome.status()` in a disciplined way.
//! - Rates are now passed using the new `Rate` structure, instead of as
//!   tuples.
//! - Timing is now perfectly accurate, instead of "only" having nanosecond
//!   precision.
//! - `Status` has been renamed to `Reading`.
//! - `Reading::Idle` now directly gives you the wait time as a `Duration`,
//!   instead of making you go indirectly through the `metronome`.
//!
//! # License
//!
//! `ftvf` is distributed under the zlib license. The complete text is as
//! follows:
//!
//! > Copyright (c) 2019, 2023 Solra Bizna
//! > 
//! > This software is provided "as-is", without any express or implied
//! > warranty. In no event will the author be held liable for any damages
//! > arising from the use of this software.
//! > 
//! > Permission is granted to anyone to use this software for any purpose,
//! > including commercial applications, and to alter it and redistribute it
//! > freely, subject to the following restrictions:
//! > 
//! > 1. The origin of this software must not be misrepresented; you must not
//! > claim that you wrote the original software. If you use this software in a
//! > product, an acknowledgement in the product documentation would be
//! > appreciated but is not required.
//! > 2. Altered source versions must be plainly marked as such, and must not
//! > be misrepresented as being the original software.
//! > 3. This notice may not be removed or altered from any source
//! > distribution.

#![cfg_attr(feature="no_std",no_std)]

// Do link to `std` if we're testing. This has to be top level instead of in
// the test module because `#[macro_use]`, applied to an `extern crate`, is
// only allowed at the top level.
#[cfg(feature="no_std")] #[macro_use]
extern crate std;

use core::{
    num::NonZeroU32,
    time::Duration,
};

#[cfg(not(feature="no_std"))]
mod realtime;
#[cfg(not(feature="no_std"))]
pub use realtime::RealtimeNowSource;

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

/// A frequency, measured by a ratio of seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Rate {
    /// Ticks.
    numerator: NonZeroU32,
    /// Seconds.
    denominator: NonZeroU32,
    duration_per: Duration,
    /// The fraction of a nanosecond that accumulates each tick. The numerator
    /// is `residual_per` and the *denominator* is `numerator`.
    residual_per: u32,
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

/// The meat of the crate. Contains all state necessary to turn pure temporal
/// chaos into an orderly stream of ticks and frames.
///
/// See the crate-level documentation for more information.
#[derive(Debug, Clone)]
pub struct Metronome<N: NowSource> {
    now_source: N,
    last_tick: Option<N::Instant>,
    last_frame: Option<N::Instant>,
    tick_residual: u32,
    frame_residual: u32,
    tickrate: Rate,
    last_framerate: Option<Rate>,
    max_ticks_behind: u32,
}

/// Time handling information returned by a
/// [`Metronome`](struct.Metronome.html).
#[derive(Clone,Copy,Debug,PartialEq)]
pub enum Reading {
    /// You should perform a logic tick.
    Tick,
    /// You should render a frame.
    Frame {
        /// Indicates where in time we are. In the range 0 (previous tick) to
        /// 1 (current tick), inclusive.
        phase: f32
    },
    /// No `Tick` or `Frame` occurred this sample. You may want to call
    /// `sleep_until_next_tick`.
    Idle {
        /// Indicates how long you need to sleep before it will be time for
        /// another tick or frame.
        duration: Duration,
    },
    /// The [`NowSource`](trait.NowSource.html) reported a timestamp strictly
    /// earlier than a previous timestamp. This should never happen. A temporal
    /// anomaly is likely. This should be handled by showing some sort of
    /// warning, or ignored.
    TimeWentBackwards,
    /// Time is passing more quickly than we can process ticks; specifically,
    /// more than the [`Metronome`](struct.Metronome.html)'s `max_ticks_behind`
    /// ticks worth of time has passed since the last time we finished a batch
    /// of ticks. This should be handled by showing some sort of warning, or
    /// ignored.
    TicksLost,
}

#[deprecated(since="0.6.0", note="use Reading instead")]
pub type Status = Reading;

/// How ticks and frames should relate to one another in a given call to
/// [`Metronome::sample`](struct.Metronome.html#method.sample).
#[derive(Clone,Copy,Debug,PartialEq)]
pub enum Mode {
    /// No rendering is happening. Good for dedicated servers, logic test
    /// suites, minimized games, and other headless applications. Never yields
    /// `Frame`.
    TickOnly,
    /// Try to render exactly one frame per tick. Frame phase will always be
    /// `1.0`. Frames may be skipped but will never be doubled.
    OneFramePerTick,
    /// Try to render as often as possible. This is the preferred value if you
    /// don't know the refresh rate. Frame phase will be very jittery. Always
    /// returns `Frame` exactly once per poll. **Never** returns `Idle`.
    UnlimitedFrames,
    // TODO: TargetFramesPerSecond((u32, u32))?
}

impl Mode {
    #[allow(non_upper_case_globals)]
    #[deprecated(since="0.6.0", note="use OneFramePerTick instead")]
    pub const MaxOneFramePerTick: Mode = Mode::OneFramePerTick;
}

impl<N: NowSource> Metronome<N> {
    /// Create a new `Metronome`, initialized with the given properties.
    /// - `now_source`: The [`NowSource`](trait.NowSource.html) to use.
    /// - `ticks_per_second`: The target rate of ticks per second, represented
    /// as a fraction. For example, `(20, 1)` → 20 ticks per 1 second.
    /// `(60000, 1001)` → 60000 ticks per 1001 seconds (color NTSC framerate).
    /// Even very large values are acceptable; the only problem you would have
    /// from `(u32::MAX, 1)` would be actually processing `Tick`s that quickly,
    /// and the only problem you would have from `(1, u32::MAX)` would be
    /// dying of old age waiting for your first `Tick`.
    /// - `max_ticks_behind`: The maximum number of ticks we can "fall behind"
    /// before we start dropping ticks. Increasing this value makes your game's
    /// tick pacing more steady over time, at the cost of making the play
    /// experience more miserable on computers too slow to play the game in
    /// realtime.  
    /// For a non-multiplayer application this should be fairly low, e.g. in
    /// the 1-3 range. In multiplayer, we should try harder to keep up, and a
    /// value on the order of several seconds' worth of ticks might be
    /// preferred.
    pub fn new(
        now_source: N,
        tickrate: Rate,
        max_ticks_behind: u32,
    ) -> Metronome<N> {
        Metronome {
            now_source,
            last_tick: None,
            last_frame: None,
            tickrate,
            last_framerate: None,
            tick_residual: 0,
            frame_residual: 0,
            max_ticks_behind,
        }
    }
    /// Call this from your logic loop, after checking for user input. Returns
    /// an `Iterator` of `Reading`s, describing how you should respond to any
    /// time that has passed.
    pub fn sample<'a>(&'a mut self, mode: Mode) -> impl Iterator<Item=Reading> + 'a {
        let new_framerate = match mode {
            Mode::TickOnly => None,
            Mode::OneFramePerTick => Some(self.tickrate.clone()),
            Mode::UnlimitedFrames => None,
        };
        if new_framerate != self.last_framerate {
            self.last_framerate = new_framerate;
            self.frame_residual = 0;
        }
        let now = self.now_source.now();
        MetronomeIterator::new(self, mode, now)
    }
    /// Dynamically change the tickrate. This will cause a small temporal
    /// anomaly, unless:
    ///
    /// - you *only* call this while handling a `Tick`
    /// - you *always* break the loop and poll again after changing the tickrate
    pub fn set_tickrate(&mut self, new_rate: Rate) {
        // TODO: make it only possible to do this from `Tick`, in a way that
        // doesn't also break all the tests
        if self.tickrate != new_rate {
            self.tickrate = new_rate;
            self.tick_residual = 0;
        }
    }
}

pub struct MetronomeIterator<'a, N: NowSource> {
    metronome: &'a mut Metronome<N>,
    now: N::Instant,
    mode: Mode,
    tick_at: Option<(N::Instant, u32)>,
    render_at: Option<(N::Instant, u32)>,
    idle_for: Option<Duration>,
    time_went_backwards: bool,
    ticks_given: u32,
}

fn calculate_next_tick<N: NowSource>(tickrate: &Rate, start_point: &N::Instant, in_residual: u32) -> (N::Instant, u32) {
    let tick_at = start_point.advanced_by(tickrate.duration_per);
    let new_residual = in_residual + tickrate.residual_per;
    if new_residual >= tickrate.numerator.get() {
        let new_residual = new_residual - tickrate.numerator.get();
        debug_assert!(new_residual < tickrate.numerator.get());
        (tick_at.advanced_by(Duration::from_nanos(1)), new_residual)
    } else { (tick_at, new_residual) }
}

impl<N: NowSource> MetronomeIterator<'_, N> {
    fn new(metronome: &mut Metronome<N>, mode: Mode, now: N::Instant) -> MetronomeIterator<'_, N> {
        let (time_went_backwards, next_tick_at);
        if let Some(last_tick) = metronome.last_tick.as_ref() {
            if now < *last_tick {
                time_went_backwards = true;
                next_tick_at = (now.clone(), 0);
            } else {
                time_went_backwards = false;
                next_tick_at = calculate_next_tick::<N>(&metronome.tickrate, last_tick, metronome.tick_residual);
            }
        } else {
            time_went_backwards = false;
            next_tick_at = (now.clone(), 0);
        }
        if time_went_backwards {
            metronome.last_tick = None;
            metronome.last_frame = None;
            metronome.tick_residual = 0;
            metronome.frame_residual = 0;
        }
        let render_at = match mode {
            Mode::TickOnly => None,
            Mode::OneFramePerTick => Some((now.clone(), 0)),
            Mode::UnlimitedFrames => Some((now.clone(), 0)),
        };
        let render_at = render_at.and_then(|render_at: (N::Instant, u32)| {
            // Don't render in the future
            if render_at.0 > now { return None }
            if let Some(last_frame) = metronome.last_frame.as_ref() {
                // Don't render the same frame twice
                if *last_frame == render_at.0 { return None }
            }
            return Some(render_at)
        });
        let idle_for = match mode {
            Mode::TickOnly | Mode::OneFramePerTick => {
                // will be None or Some(ZERO) if we don't need to idle
                next_tick_at.0.time_since(&now)
            },
            Mode::UnlimitedFrames => None,
        };
        let idle_for = match idle_for {
            None | Some(Duration::ZERO) => None,
            x => x,
        };
        MetronomeIterator {
            metronome,
            idle_for,
            render_at,
            tick_at: if next_tick_at.0 > now { None } else { Some(next_tick_at) },
            now,
            time_went_backwards,
            mode,
            ticks_given: 0,
        }
    }
}

impl<N: NowSource> Iterator for MetronomeIterator<'_, N> {
    type Item = Reading;
    fn next(&mut self) -> Option<Reading> {
        if self.time_went_backwards {
            self.time_went_backwards = false;
            return Some(Reading::TimeWentBackwards)
        }
        let should_render_now = match (self.tick_at.as_ref(), self.render_at.as_ref()) {
            (Some(next_tick), Some(render_at)) => render_at.0 < next_tick.0,
            (None, Some(_)) => true,
            _ => false,
        };
        if !should_render_now {
            if let Some(next_tick_at) = self.tick_at.take() {
                if self.ticks_given >= self.metronome.max_ticks_behind {
                    // we have taken away `next_tick_at`, so we won't tick anymore
                    return Some(Reading::TicksLost)
                }
                (self.metronome.last_tick, self.metronome.tick_residual)
                    = (Some(next_tick_at.0.clone()), next_tick_at.1);
                let nexter_tick_at = calculate_next_tick::<N>(&self.metronome.tickrate, &next_tick_at.0, next_tick_at.1);
                if nexter_tick_at.0 <= self.now {
                    self.tick_at = Some(nexter_tick_at);
                }
                return Some(Reading::Tick);
            }
        }
        // We got here because we didn't tick. Maybe we didn't tick because we
        // need to render.
        if let Some(render_at) = self.render_at.take() {
            (self.metronome.last_frame, self.metronome.frame_residual)
                = (Some(render_at.0.clone()), render_at.1);
            let phase = match self.mode {
                Mode::TickOnly => unreachable!(),
                Mode::OneFramePerTick => 1.0,
                Mode::UnlimitedFrames => {
                    match self.metronome.last_tick.as_ref() {
                        None => 1.0, // >:(
                        Some(last_tick) => {
                            let now_offset = render_at.0.time_since(last_tick)
                                .unwrap_or(Duration::ZERO);
                            now_offset.as_nanos() as u64 as f32
                            / self.metronome.tickrate.duration_per.as_nanos() as u64 as f32
                        },
                    }
                },
            };
            return Some(Reading::Frame { phase });
        }
        if let Some(duration) = self.idle_for.take() {
            return Some(Reading::Idle { duration });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature="no_std")]
    use std::prelude::*;
    use std::cell::RefCell;
    use super::*;
    #[derive(Debug)]
    enum TestCmd<'a> {
        SetNow(u64, u32),
        Sample(Mode, &'a[Reading]),
        SetTickrate(u32, u32),
    }
    use TestCmd::*;
    #[derive(Copy,Clone,Default,Debug,PartialOrd,PartialEq)]
    struct TestInstant(Duration);
    impl TemporalSample for TestInstant {
        fn time_since(&self, origin: &Self) -> Option<Duration> {
            self.0.checked_sub(origin.0)
        }
        fn advanced_by(&self, amount: Duration) -> Self {
            TestInstant(self.0 + amount)
        }
    }
    #[derive(Debug,Copy,Clone)]
    struct TestNowSource {
        now: TestInstant,
    }
    impl TestNowSource {
        pub fn new() -> TestNowSource {
            TestNowSource { now: Default::default() }
        }
        pub fn set_now(&mut self, delta: Duration) {
            self.now.0 = delta;
        }
    }
    impl NowSource for TestNowSource {
        type Instant = TestInstant;
        fn now(&mut self) -> TestInstant { self.now }
    }
    impl NowSource for &RefCell<TestNowSource> {
        type Instant = TestInstant;
        fn now(&mut self) -> TestInstant { self.borrow().now }
    }
    fn run_test(tps: (u32, u32), max_ticks_behind: u32, cmds: &[TestCmd]) {
        let now_source = RefCell::new(TestNowSource::new());
        let mut metronome = Metronome::new(&now_source, Rate::per_second(tps.0, tps.1), max_ticks_behind);
        let mut bad = None;
        for n in 0..cmds.len() {
            let cmd = &cmds[n];
            match cmd {
                SetNow(sec, nsec) => {
                    now_source.borrow_mut().set_now(Duration::new(*sec,*nsec));
                },
                Sample(mode, readings) => {
                    let check: Vec<Reading> = metronome.sample(*mode).collect();
                    if &check[..] != *readings {
                        bad = Some((n, format!("got {:?}", check)));
                        break;
                    }
                },
                SetTickrate(num, den) => {
                    metronome.set_tickrate(Rate::per_second(*num, *den));
                },
            }
        }
        if let Some((index, explanation)) = bad {
            eprintln!("Test failed!");
            for n in index.saturating_sub(10) .. index {
                eprintln!("OK\t[{}] = {:?}", n, cmds[n]);
            }
            eprintln!("BAD\t[{}] = {:?}", index, cmds[index]);
            eprintln!("{}", explanation);
            panic!("Test failed!");
        }
    }
    #[test]
    fn simple() {
        const IDLE_FIFTH_SECOND: &[Reading] = &[
            Reading::Idle { duration: Duration::from_millis(200) },
        ];
        run_test((5, 1), 10, &[
            Sample(Mode::OneFramePerTick, &[
                Reading::Tick,
                Reading::Frame { phase: 1.0 },
            ]),
            Sample(Mode::OneFramePerTick, IDLE_FIFTH_SECOND),
            SetNow(1, 0),
            Sample(Mode::UnlimitedFrames, &[
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Frame { phase: 0.0 },
            ]),
            Sample(Mode::UnlimitedFrames, &[
            ]),
            SetNow(2, 0),
            Sample(Mode::TickOnly, &[
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
            ]),
            Sample(Mode::TickOnly, IDLE_FIFTH_SECOND),
            SetNow(2, 100000000),
            Sample(Mode::UnlimitedFrames, &[
                Reading::Frame { phase: 0.5 },
            ]),
            Sample(Mode::UnlimitedFrames, &[
            ]),
            SetNow(2, 200000000),
            Sample(Mode::UnlimitedFrames, &[
                Reading::Tick,
                Reading::Frame { phase: 0.0 },
            ]),
            SetNow(2, 0),
            Sample(Mode::UnlimitedFrames, &[
                Reading::TimeWentBackwards,
                Reading::Tick,
                Reading::Frame { phase: 0.0 },
            ]),
        ]);
    }
    #[test]
    fn ntsc() {
        run_test((60000, 1001), 120, &[
            Sample(Mode::UnlimitedFrames, &[
                Reading::Tick,
                Reading::Frame { phase: 0.0 },
            ]),
            SetNow(0, 500000000),
            Sample(Mode::UnlimitedFrames, &[
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Tick,
                Reading::Frame { phase: 0.97003 }, // roughly 30.0 / 1.001 - 29.0
            ]),
        ]);
    }
    #[test]
    fn residual_tick() {
        run_test((3,1), 444, &[
            Sample(Mode::OneFramePerTick, &[
                Reading::Tick,
                Reading::Frame { phase: 1.0 },
            ]),
            SetNow(0, 500000000),
            Sample(Mode::OneFramePerTick, &[
                Reading::Tick,
                Reading::Frame { phase: 1.0 },
            ]),
            Sample(Mode::OneFramePerTick, &[
                Reading::Idle { duration: Duration::from_nanos(166666666) },
            ]),
            SetNow(0, 750000000),
            Sample(Mode::OneFramePerTick, &[
                Reading::Tick,
                Reading::Frame { phase: 1.0 },
            ]),
            Sample(Mode::OneFramePerTick, &[
                Reading::Idle { duration: Duration::from_nanos(250000000) },
            ]),
        ]);
    }
}
