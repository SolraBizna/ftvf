use core::time::Duration;

use super::{NowSource, Rate, TemporalSample};

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
    /// `1.0`.
    OneFramePerTick,
    /// Try to render as often as possible. This is the preferred value if you
    /// don't know the refresh rate. Frame phase will be very jittery.
    /// **Never** returns `Idle`.
    UnlimitedFrames,
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
    /// time that has passed. See [`Reading`](enum.Reading.html) for info on
    /// what each reading means.
    pub fn sample<'a>(&'a mut self, mode: Mode) -> impl Iterator<Item=Reading> + 'a {
        let new_framerate = match mode {
            Mode::TickOnly => None,
            Mode::OneFramePerTick => Some(self.tickrate.clone()),
            Mode::UnlimitedFrames => None,
        };
        if new_framerate != self.last_framerate {
            self.last_framerate = new_framerate;
            self.last_frame = None;
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
