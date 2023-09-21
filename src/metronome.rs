use core::time::Duration;

use super::{NowSource, PreciseInstant, Rate, TemporalSample};

/// The meat of the crate. Contains all state necessary to turn pure temporal
/// chaos into an orderly stream of ticks and frames.
///
/// See the crate-level documentation for more information.
#[derive(Debug, Clone)]
pub struct Metronome<N: NowSource> {
    now_source: N,
    past_tick: Option<PreciseInstant<N::Instant>>,
    future_tick: Option<PreciseInstant<N::Instant>>,
    last_frame: Option<PreciseInstant<N::Instant>>,
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
    /// No `Tick` or `Frame` occurred this sample. If you call
    /// `std::thread::sleep(duration)` (or equivalent) and then sample again,
    /// you will have waited exactly long enough for the next `Tick` or `Frame`
    /// to appear.
    Idle {
        /// Indicates how long you need to sleep before it will be time for
        /// another tick or frame.
        duration: Duration,
    },
    /// The [`NowSource`](trait.NowSource.html) reported a timestamp strictly
    /// earlier than a previous timestamp. This should never happen. A temporal
    /// anomaly has happened. This should be handled by showing some sort of
    /// warning, or ignored.
    ///
    /// `ftvf` currently fails to detect temporal anomalies that result in one
    /// tick or less of "slip".
    TimeWentBackwards,
    /// Time is passing more quickly than we can process ticks; specifically,
    /// more than the [`Metronome`](struct.Metronome.html)'s `max_ticks_behind`
    /// ticks worth of time has passed since the last time we finished a batch
    /// of ticks. This should be handled by showing some sort of warning, or
    /// ignored.
    TicksLost,
}

#[deprecated(since="0.6.0", note="use Reading instead")]
#[doc(hidden)]
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
    /// `1.0`.
    OneFramePerTick,
    /// Try to render as often as possible. This is the preferred value if you
    /// don't know the refresh rate. Frame phase will be very jittery.
    /// **Never** returns `Idle`.
    UnlimitedFrames,
    /// Try to render at the given target framerate. This is the preferred
    /// value if you *do* know the refresh rate. Frame phase will be very
    /// regular, especially if there is a simple relationship between tickrate
    /// and framerate.
    TargetFramesPerSecond(Rate),
}

impl Mode {
    #[allow(non_upper_case_globals)]
    #[deprecated(since="0.6.0", note="use OneFramePerTick instead")]
    #[doc(hidden)]
    pub const MaxOneFramePerTick: Mode = Mode::OneFramePerTick;
    fn needs_a_future(&self) -> bool {
        match self {
            Mode::UnlimitedFrames | Mode::TargetFramesPerSecond(_) => true,
            _ => false,
        }
    }
}

impl<N: NowSource> Metronome<N> {
    /// Create a new `Metronome`, initialized with the given properties.
    /// - `now_source`: The [`NowSource`](trait.NowSource.html) to use.
    /// - `tickrate`: The target rate of ticks per second, represented as a
    ///   [`Rate`](struct.Rate.html).
    /// - `max_ticks_behind`: The maximum number of ticks we can "fall behind"
    ///   before we start dropping ticks. Increasing this value makes your
    ///   game's tick pacing more steady over time, at the cost of making the
    ///   play experience more miserable on computers too slow to play the game
    ///   in realtime.  
    ///   For a non-multiplayer application this should be fairly low, e.g. in
    ///   the 1-3 range. In multiplayer, we should try harder to keep up, and a
    ///   value on the order of several seconds' worth of ticks might be
    ///   preferred.
    pub fn new(
        now_source: N,
        tickrate: Rate,
        max_ticks_behind: u32,
    ) -> Metronome<N> {
        Metronome {
            now_source,
            past_tick: None,
            future_tick: None,
            last_frame: None,
            tickrate,
            last_framerate: None,
            max_ticks_behind,
        }
    }
    /// Call this from your logic loop, after checking for user input. Returns
    /// an `Iterator` of `Reading`s, describing how you should respond to the
    /// passage of time. See [`Reading`](enum.Reading.html) for info on what
    /// each reading means.
    pub fn sample<'a>(&'a mut self, mode: Mode) -> impl Iterator<Item=Reading> + 'a {
        let new_framerate = match mode {
            Mode::TickOnly => None,
            Mode::OneFramePerTick => Some(self.tickrate.clone()),
            Mode::UnlimitedFrames => None,
            Mode::TargetFramesPerSecond(rate) => Some(rate.clone()),
        };
        if new_framerate != self.last_framerate {
            self.last_framerate = new_framerate;
            self.last_frame = None;
        }
        let now = self.now_source.now();
        MetronomeIterator::new(self, mode, now)
    }
    /// Dynamically change the tickrate. You can call this at any time and it
    /// will take effect after the current tick. If you call this from within
    /// a loop over an iterator returned by `sample`, you should `break` out of
    /// the loop, because it does not currently detect the new tick rate
    /// mid-loop.
    pub fn set_tickrate(&mut self, new_rate: Rate) {
        if self.tickrate != new_rate {
            self.tickrate = new_rate;
            if let Some(past_tick) = self.past_tick.as_mut() {
                past_tick.forget_residual();
            }
            if let Some(future_tick) = self.future_tick.as_mut() {
                future_tick.forget_residual();
            }
        }
    }
}

/// Returned by [`Metronome::sample`](struct.Metronome.html#method.sample). See
/// that method's documentation.

pub struct MetronomeIterator<'a, N: NowSource> {
    metronome: &'a mut Metronome<N>,
    now: N::Instant,
    mode: Mode,
    tick: Option<PreciseInstant<N::Instant>>,
    frame: Option<PreciseInstant<N::Instant>>,
    idle_for: Option<Duration>,
    time_went_backwards: bool,
    ticks_given: u32,
}

impl<N: NowSource> MetronomeIterator<'_, N> {
    fn new(metronome: &mut Metronome<N>, mode: Mode, now: N::Instant) -> MetronomeIterator<'_, N> {
        let mut time_went_backwards = false;
        if let Some(past_tick) = metronome.past_tick.as_ref() {
            if now < past_tick.at {
                time_went_backwards = true;
                metronome.past_tick = None;
                metronome.future_tick = None;
                metronome.last_frame = None;
            }
        }
        let tick = if let Some(future_tick) = metronome.future_tick.as_ref() {
            future_tick.next(&metronome.tickrate)
        } else {
            PreciseInstant::from(now.clone())
        };
        let frame = match mode {
            Mode::TickOnly => None,
            Mode::OneFramePerTick => {
                Some(tick.last_tick_before(&now, &metronome.tickrate))
            },
            Mode::UnlimitedFrames => Some(PreciseInstant::from(now.clone())),
            Mode::TargetFramesPerSecond(rate) => {
                debug_assert_eq!(Some(rate), metronome.last_framerate);
                match metronome.last_frame.as_ref() {
                    Some(last_frame) => Some(last_frame.last_tick_before(&now, &rate)),
                    None => Some(tick.last_tick_before(&now, &metronome.tickrate)),
                }
            },
        };
        let frame = frame.and_then(|frame| {
            if frame.at > now {
                // Don't render a frame in the future
                return None
            } else if let Some(last_frame) = metronome.last_frame.as_ref() {
                // Don't render the same frame twice
                if *last_frame == frame { return None }
            }
            Some(frame)
        });
        let idle_for = match mode {
            Mode::TickOnly | Mode::OneFramePerTick => {
                // will be None or Some(ZERO) if we don't need to idle
                tick.at.time_since(&now)
            },
            Mode::TargetFramesPerSecond(rate) => {
                let a = tick.at.time_since(&now);
                let b = frame.as_ref().map(Clone::clone).unwrap_or_else(|| {
                    metronome.last_frame.as_ref().unwrap().next(&rate)
                }).at.time_since(&now);
                match (a, b) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    _ => None,
                }
            },
            Mode::UnlimitedFrames => None,
        };
        let idle_for = match idle_for {
            None | Some(Duration::ZERO) => None,
            x => x,
        };
        let want_future = if let Some(_frame) = frame.as_ref() {
            mode.needs_a_future()
        } else { false };
        let tick = if want_future || tick.at <= now {
            Some(tick)
        } else { None };
        MetronomeIterator {
            idle_for,
            frame,
            tick,
            metronome,
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
        let should_render_now = match (self.tick.as_ref(), self.frame.as_ref()) {
            (Some(_tick), Some(frame)) => {
                match (self.metronome.past_tick.as_ref(), self.metronome.future_tick.as_ref()) {
                    (Some(past), Some(future)) => frame >= past && frame <= future,
                    _ => false,
                }
            },
            (None, Some(_)) => true,
            _ => false,
        };
        if !should_render_now {
            if let Some(tick) = self.tick.take() {
                if tick.at <= self.now || self.frame.is_some() {
                    if self.ticks_given >= self.metronome.max_ticks_behind {
                        // Enough ticks have been delivered. Complain.
                        self.metronome.past_tick = None;
                        self.metronome.future_tick = None;
                        self.metronome.last_frame = None;
                        // self.tick has already been None'd
                        // self.frame may (or may not) lead to us eventually
                        // rendering
                        return Some(Reading::TicksLost)
                    }
                    self.metronome.past_tick = self.metronome.future_tick.take();
                    self.metronome.future_tick = Some(tick.clone());
                    if self.metronome.past_tick.is_none() {
                        self.metronome.past_tick = self.metronome.future_tick.clone();
                    }
                    let tick = tick.next(&self.metronome.tickrate);
                    self.tick = Some(tick);
                    return Some(Reading::Tick);
                }
            }
        }
        // We got here because we didn't tick. Maybe we didn't tick because we
        // need to render.
        if let Some(frame) = self.frame.take() {
            let phase = match self.mode {
                Mode::TickOnly => unreachable!(),
                Mode::OneFramePerTick => 1.0,
                Mode::UnlimitedFrames | Mode::TargetFramesPerSecond(_) => {
                    match (self.metronome.past_tick.as_ref(), self.metronome.future_tick.as_ref()) {
                        (Some(past_tick), Some(future_tick)) if past_tick != future_tick => {
                            if frame.at < past_tick.at { 0.0 }
                            else if frame.at > future_tick.at { 1.0 }
                            else {
                                let tick_step = future_tick.at.time_since(&past_tick.at).unwrap();
                                let frame_offset = frame.at.time_since(&past_tick.at).unwrap();
                                frame_offset.as_nanos() as f32 / tick_step.as_nanos() as f32
                            }
                        },
                        _ => 1.0,
                    }
                },
            };
            self.metronome.last_frame = Some(frame);
            // if we render, do not tick again
            self.tick = None;
            return Some(Reading::Frame { phase });
        }
        if let Some(duration) = self.idle_for.take() {
            return Some(Reading::Idle { duration });
        }
        None
    }
}
