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
//! ftvf = "0.5"
//! ```
//! 
//! then initialize yourself a [`Metronome`](struct.Metronome.html):
//!
//! ```rust
//! # use ftvf::*;
//! let mut metronome = Metronome::new(RealtimeNowSource::new(),
//!                                    (30, 1), // want 30 ticks per 1 second
//!                                    5); // accept being up to 5 ticks behind
//! ```
//!
//! And then your game loop looks like this:
//!
//! ```rust
//! # use ftvf::*;
//! # struct GameWorld {}
//! # impl GameWorld {
//! #   fn handle_input(&mut self) {}
//! #   fn perform_tick(&mut self) {}
//! #   fn render(&mut self, _: f32) {}
//! #   fn should_quit(&mut self) -> bool { true }
//! # }
//! # let mut metronome = Metronome::new(RealtimeNowSource::new(), (30,1), 5);
//! # let mut world = GameWorld{};
//! while !world.should_quit() {
//!   world.handle_input();
//!   // call `sample` once per batch. not zero times, not two or more times!
//!   metronome.sample();
//!   while let Some(status) = metronome.status(Mode::UnlimitedFrames) {
//!     match status {
//!       Status::Tick => world.perform_tick(),
//!       Status::Frame{phase} => world.render(phase),
//!       Status::TimeWentBackwards
//!         => eprintln!("Warning: time flowed backwards!"),
//!       Status::TicksLost(n)
//!         => eprintln!("Warning: we're too slow, lost {} ticks!", n),
//!       // No special handling or warning message is needed for Rollover. In
//!       // practice, it will never be seen.
//!       Status::Rollover => (),
//!       // Mode::UnlimitedFrames never returns Idle, but other modes can, and
//!       // this is the way it should be handled.
//!       Status::Idle => metronome.sleep_until_next_tick(),
//!     }
//!   }
//! }
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
//! # License
//!
//! `ftvf` is distributed under the zlib license. The complete text is as
//! follows:
//!
//! > Copyright (c) 2019, Solra Bizna
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
use std::time::{Duration,Instant};

/// A source of time information for [`Metronome`](struct.Metronome.html) to
/// use. For most purposes,
/// [`RealtimeNowSource`](struct.RealtimeNowSource.html) will be sufficient.
pub trait NowSource : Copy {
    /// Return an `Instant` representing *now*.
    fn now(&mut self) -> Instant;
    /// Sleep until at least `how_long` from *now*. Optional.
    ///
    /// Will only be called very soon after `now()`, so no need to specially
    /// account for "temporal slippage".
    #[allow(unused)]
    fn sleep(&mut self, how_long: Duration) {}
}

/// A [`NowSource`](trait.NowSource.html) that uses the standard Rust timing
/// facilities to obtain its timing information. This is the default
/// `NowSource`, and also the one you almost certainly want to use.
#[derive(Debug,Copy,Clone)]
pub struct RealtimeNowSource {}

impl RealtimeNowSource {
    pub fn new() -> RealtimeNowSource { RealtimeNowSource { } }
}

impl NowSource for RealtimeNowSource {
    fn now(&mut self) -> Instant { Instant::now() }
    fn sleep(&mut self, how_long: Duration) { std::thread::sleep(how_long) }
}

/// The meat of the crate. Contains all state necessary to turn pure temporal
/// chaos into an orderly stream of ticks and frames.
///
/// See the crate-level documentation for more information.
#[derive(Debug,Copy,Clone)]
pub struct Metronome<N: NowSource = RealtimeNowSource> {
    now_source: N,
    epoch: Instant,
    now: Instant,
    ticks_per_second: (u32, u32),
    max_ticks_behind: u32,
    last_tick_no: u64,
    rendered_this_tick: bool,
    rendered_this_sample: bool,
    return_idle: bool,
    paused: bool,
}

/// Time handling information returned by a
/// [`Metronome`](struct.Metronome.html).
#[derive(Clone,Copy,Debug,PartialEq)]
pub enum Status {
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
    Idle,
    /// The [`NowSource`](trait.NowSource.html) reported a timestamp strictly
    /// earlier than a previous timestamp. This should never happen. A temporal
    /// anomaly is likely. This should be handled by showing some sort of
    /// warning, or ignored.
    ///
    /// This may also occur when switching [`Mode`](enum.Mode.html)s on the
    /// same [`Metronome`](struct.Metronome.html) from `TicksOnly` to another
    /// mode, which usually would not happen.
    TimeWentBackwards,
    /// Time is passing more quickly than we can process ticks; specifically,
    /// more than the [`Metronome`](struct.Metronome.html)'s `max_ticks_behind`
    /// ticks worth of time has passed since the last time we finished a batch
    /// of ticks. This should be handled by showing some sort of warning, or
    /// ignored.
    ///
    /// The value is the number of ticks' worth of time that were just lost.
    TicksLost(u64),
    /// An obscenely huge amount of time has passed, and a rarely-used piece of
    /// logic within `ftvf` handled it correctly. You should **ignore this**
    /// unless you're testing `ftvf`.
    ///
    /// In a typical application, the amount of time necessary to produce this
    /// variant is on the order of **18,000,000,000 years**. Even in the most
    /// extreme case (2³²-1 ticks per second), over 136 years must pass for
    /// `Rollover` to occur. Unless your application is going to operate
    /// **continuously** for that kind of time frame, you will never encounter
    /// a `Rollover`; and even if it does, the fact that you did merely
    /// indicates that `ftvf` is handling the case correctly and nothing needs
    /// to be done on your end.
    Rollover,
}

/// How ticks and frames should relate to one another in a given call to
/// [`Metronome::status`](struct.Metronome.html#method.status).
#[derive(Clone,Copy,Debug,PartialEq)]
pub enum Mode {
    /// No rendering is happening. `Metronome::status` will return `None` when
    /// all ticks in the current batch are finished. Good for dedicated
    /// servers, logic test suites, and other headless applications.
    TickOnly,
    /// Only render at most one frame per tick.
    MaxOneFramePerTick,
    /// May render an unlimited number of frames between ticks. This is the
    /// preferred value, especially when the intended tickrate is substantially
    /// lower than the intended framerate. **Never returns `Idle`.**
    UnlimitedFrames,
    // TODO: TargetFramesPerSecond((u32, u32))?
}

impl Mode {
    fn cares_about_subticks(&self) -> bool {
        *self != Mode::TickOnly
    }
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
    /// before we start dropping ticks. For a non-multiplayer application this
    /// should be fairly low, e.g. in the 1-3 range. In multiplayer, we should
    /// try harder to keep up, and a value on the order of several seconds'
    /// worth of ticks would be preferred.
    pub fn new(mut now_source: N,
               ticks_per_second: (u32, u32),
               max_ticks_behind: u32) -> Metronome<N> {
        assert_ne!(ticks_per_second.0, 0);
        assert_ne!(ticks_per_second.1, 0);
        let epoch = now_source.now();
        Metronome {
            now_source,
            epoch,
            now: epoch,
            ticks_per_second,
            max_ticks_behind,
            last_tick_no: 0,
            rendered_this_tick: false,
            rendered_this_sample: false,
            return_idle: true,
            paused: false,
        }
    }
    /// Take a temporal sample. This should be called before each batch of
    /// `status` calls.
    pub fn sample(&mut self) {
        self.now = self.now_source.now();
        self.rendered_this_sample = false;
        self.return_idle = true;
    }
    /// Advance the epoch to the latest tick, to handle rollover or a tickrate
    /// change. We always put this off as long as possible, giving us an absurd
    /// amount of precision on the operation.
    fn advance_epoch(&mut self) {
        self.epoch += Duration::new(self.last_tick_no, 0)
            * self.ticks_per_second.1 / self.ticks_per_second.0;
        self.last_tick_no = 0;
    }
    /// Overflow, either because we've been running a really really long time
    /// or because we have a really huge numerator on our tickrate.
    fn rollover(&mut self) {
        if self.last_tick_no == 0 {
            // This should never happen; even if the tickrate was
            // `(u32::MAX, 1)`, there should still be time for billions
            // of seconds of ticks before overflow.
            unreachable!();
        }
        self.advance_epoch()
    }
    /// Call in a loop after calling `sample`. Returns the actions that you
    /// should take to advance your game world, possibly interspersed with
    /// status information about unusual temporal conditions.
    pub fn status(&mut self, mode: Mode) -> Option<Status> {
        // first off: if Now is before Epoch, time has REALLY flowed backward
        if self.now < self.epoch {
            self.epoch = self.now;
            self.last_tick_no = 0;
            return Some(Status::TimeWentBackwards)
        }
        // calculate the number of ticks between Epoch and Now
        let time_since_epoch = self.now - self.epoch;
        let duration_since_epoch = match time_since_epoch.checked_mul(self.ticks_per_second.0) {
            Some(x) => x,
            None => {
                self.rollover();
                return Some(Status::Rollover)
            },
        } / self.ticks_per_second.1;
        // (if necessary, send this back by one tick...)
        let (ticks_since_epoch, subsec) = if duration_since_epoch.subsec_nanos() == 0 && mode.cares_about_subticks() {
            (duration_since_epoch.as_secs().saturating_sub(1), 1000000000)
        }
        else {
            (duration_since_epoch.as_secs(),
             duration_since_epoch.subsec_nanos())
        };
        // if it's lower than the last number, time has flowed backward
        if ticks_since_epoch < self.last_tick_no {
            self.last_tick_no = ticks_since_epoch;
            return Some(Status::TimeWentBackwards)
        }
        // how many ticks since the last one?
        let ticks_since_last = ticks_since_epoch - self.last_tick_no;
        if ticks_since_last > self.max_ticks_behind as u64 {
            let lost_ticks = ticks_since_last - 1;
            self.last_tick_no += lost_ticks;
            return Some(Status::TicksLost(lost_ticks))
        }
        else if ticks_since_last > 0 {
            self.last_tick_no += 1;
            self.rendered_this_tick = false;
            self.return_idle = false;
            if !self.paused {
                return Some(Status::Tick)
            }
        }
        match mode {
            Mode::TickOnly => {
                if self.return_idle {
                    self.return_idle = false;
                    Some(Status::Idle)
                }
                else { None }
            },
            Mode::MaxOneFramePerTick => {
                if self.rendered_this_tick {
                    if self.return_idle {
                        self.return_idle = false;
                        Some(Status::Idle)
                    }
                    else { None }
                }
                else {
                    self.rendered_this_tick = true;
                    self.return_idle = false;
                    Some(Status::Frame { phase: 1.0 })
                }
            },
            Mode::UnlimitedFrames => {
                if self.rendered_this_sample { None }
                else {
                    self.rendered_this_sample = true;
                    self.return_idle = false;
                    if self.paused {
                        Some(Status::Frame { phase: 1.0 })
                    }
                    else {
                        Some(Status::Frame { phase: (subsec as f32) / 1.0e9 })
                    }
                }
            }
        }
    }
    /// Return the exact amount that you should sleep, starting at the last
    /// temporal sample, to arrive at the moment of the next tick. You should
    /// usually call `sleep_until_next_tick` instead unless you're testing
    /// `ftvf`. See that method for other information.
    pub fn amount_to_sleep_until_next_tick(&mut self) -> Option<Duration> {
        let duration_from_epoch_until_next_tick
            = match Duration::new(self.last_tick_no+1, 0)
            .checked_mul(self.ticks_per_second.1) {
                Some(x) => x / self.ticks_per_second.0,
                None => {
                    self.rollover();
                    Duration::new(1, 0) * self.ticks_per_second.1
                        / self.ticks_per_second.0
                }
            };
        let moment_of_next_tick = self.epoch + duration_from_epoch_until_next_tick;
        if moment_of_next_tick > self.now {
            Some(moment_of_next_tick - self.now)
        }
        else {
            None
        }
    }
    /// Assuming that the current time is fairly close to the most recent
    /// temporal sample, sleep until the moment of the next tick. Good for
    /// saving CPU time on mobile devices / dedicated servers. You should only
    /// call this in response to an `Idle` return from `sample`.
    pub fn sleep_until_next_tick(&mut self) {
        match self.amount_to_sleep_until_next_tick() {
            None => (),
            Some(x) => self.now_source.sleep(x)
        }
    }
    /// Pauses (or unpauses) time. When time is paused, time is being eaten; no
    /// `Tick`s occur, but `Frame`s may still occur as normal. When resumed,
    /// `Tick`s will start again, as though no time had passed.
    pub fn set_paused(&mut self, paused: bool) { self.paused = paused }
    /// Dynamically change the tickrate. This can be called during the handling
    /// of a `Tick`, and should not be called at other times, lest temporal
    /// anomalies occur.
    pub fn set_tickrate(&mut self, ticks_per_second: (u32, u32)) {
        if ticks_per_second != self.ticks_per_second {
            self.advance_epoch();
            debug_assert_eq!(self.last_tick_no, 0);
            self.ticks_per_second = ticks_per_second;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use super::*;
    #[derive(Debug)]
    enum TestCmd {
        SetNow(u64, u32),
        StatusWithMode(Option<Status>, Mode),
        SetTickrate(u32, u32),
        SetPaused(bool),
        AmountToSleep(u64, u32),
        ShouldNotSleep,
    }
    use TestCmd::*;
    #[derive(Debug,Copy,Clone)]
    struct TestNowSource {
        epoch: Instant,
        now: Instant,
    }
    impl TestNowSource {
        pub fn new() -> TestNowSource {
            let now = Instant::now();
            TestNowSource { epoch: now, now }
        }
        pub fn set_now(&mut self, delta: Duration) {
            self.now = self.epoch.checked_add(delta).unwrap();
        }
    }
    impl NowSource for TestNowSource {
        fn now(&mut self) -> Instant { self.now }
    }
    impl NowSource for &RefCell<TestNowSource> {
        fn now(&mut self) -> Instant { self.borrow().now }
    }
    fn run_test(tps: (u32, u32), max_ticks_behind: u32, cmds: &[TestCmd]) {
        let now_source = RefCell::new(TestNowSource::new());
        let mut metronome = Metronome::new(&now_source, tps, max_ticks_behind);
        let mut bad = None;
        for n in 0..cmds.len() {
            let cmd = &cmds[n];
            match cmd {
                SetNow(sec, nsec) => {
                    now_source.borrow_mut().set_now(Duration::new(*sec,*nsec));
                    metronome.sample();
                },
                StatusWithMode(status, mode) => {
                    let check = metronome.status(*mode);
                    if status != &check {
                        // (allow for different floating point error properties
                        // of the different way of calculating subframes)
                        let ok = if let Some(Status::Frame{phase}) = *status {
                            if let Some(Status::Frame{phase:check_phase}) = check {
                                (phase - check_phase).abs() < 0.0001
                            }
                            else { false }
                        } else { false };
                        if !ok {
                            bad = Some((n, format!("expected {:?}, got {:?}", status, check)));
                            break;
                        }
                    }
                },
                AmountToSleep(sec, nsec) => {
                    let duration = Some(Duration::new(*sec, *nsec));
                    let check = metronome.amount_to_sleep_until_next_tick();
                    if duration != check {
                        bad = Some((n, format!("expected {:?}, got {:?}", duration, check)));
                        break;
                    }
                },
                ShouldNotSleep => {
                    let check = metronome.amount_to_sleep_until_next_tick();
                    if None != check {
                        bad = Some((n, format!("expected None, got {:?}", check)));
                        break;
                    }
                },
                SetTickrate(num, den) => {
                    metronome.set_tickrate((*num, *den));
                },
                SetPaused(paused) => { metronome.set_paused(*paused) },
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
        run_test((5, 1), 10, &[
            AmountToSleep(0, 200000000),
            SetNow(1, 0),
            ShouldNotSleep,
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Frame{phase:1.0}), Mode::UnlimitedFrames),
            StatusWithMode(None, Mode::UnlimitedFrames),
            SetNow(2, 0),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(None, Mode::TickOnly),
            SetNow(2, 100000000),
            StatusWithMode(Some(Status::Frame{phase:0.5}), Mode::UnlimitedFrames),
            StatusWithMode(None, Mode::UnlimitedFrames),
            SetNow(2, 200000000),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(None, Mode::TickOnly),
            // Time goes backward because UnlimitedFrames can wind up "one tick
            // behind" on exact instants where the clock lines up with the
            // tickrate. This may or may not be a bug. I think it is, but is
            // not worth fixing, because it would complicate the loop for a
            // case that will rarely happen and never cause problems (unless
            // your program handles TimeWentBackwards by raising an error); and
            // for pity's sakes, we already handled rollover! What more do you
            // want!?
            StatusWithMode(Some(Status::TimeWentBackwards), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Frame{phase:1.0}), Mode::UnlimitedFrames),
            StatusWithMode(None, Mode::UnlimitedFrames),
            SetNow(2, 400000000),
            StatusWithMode(Some(Status::Tick), Mode::MaxOneFramePerTick),
            StatusWithMode(Some(Status::Frame{phase:1.0}), Mode::MaxOneFramePerTick),
            StatusWithMode(None, Mode::MaxOneFramePerTick),
            StatusWithMode(Some(Status::Frame{phase:1.0}), Mode::UnlimitedFrames),
            StatusWithMode(None, Mode::MaxOneFramePerTick),
            StatusWithMode(None, Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            SetNow(4, 400000000),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            SetNow(6, 600000000),
            StatusWithMode(Some(Status::TicksLost(10)), Mode::TickOnly),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(None, Mode::TickOnly),
            SetNow(6, 0),
            StatusWithMode(Some(Status::TimeWentBackwards), Mode::TickOnly),
            StatusWithMode(Some(Status::Idle), Mode::TickOnly),
            StatusWithMode(None, Mode::TickOnly),
            SetNow(6, 200000000),
            StatusWithMode(Some(Status::Frame{phase:1.0}), Mode::MaxOneFramePerTick),
            StatusWithMode(None, Mode::MaxOneFramePerTick),
            StatusWithMode(Some(Status::Frame{phase:1.0}), Mode::UnlimitedFrames),
            StatusWithMode(None, Mode::MaxOneFramePerTick),
        ]);
    }
    #[test]
    fn rollover() {
        run_test((0x10000, 1), 10, &[
            SetNow(0, 0),
            StatusWithMode(Some(Status::Idle), Mode::TickOnly),
            StatusWithMode(None, Mode::TickOnly),
            SetNow(0x100000000, 0),
            StatusWithMode(Some(Status::TicksLost(0x1000000000000-1)), Mode::TickOnly),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(None, Mode::TickOnly),
            SetNow(0x1000000000000, 0),
            StatusWithMode(Some(Status::Rollover), Mode::TickOnly),
            StatusWithMode(Some(Status::TicksLost((0x1000000000000000-0x100000000000)*16-1)), Mode::TickOnly),
            StatusWithMode(Some(Status::Tick), Mode::TickOnly),
            StatusWithMode(None, Mode::TickOnly),
        ]);
    }
    #[test] #[should_panic]
    fn rollover_crash() {
        run_test((0x10000, 1), 10, &[
            SetNow(0, 0),
            StatusWithMode(None, Mode::TickOnly),
            SetNow(0x1000000000000, 0),
            StatusWithMode(None, Mode::TickOnly),
        ]);
    }
    #[test]
    fn ntsc() {
        run_test((60000, 1001), 120, &[
            StatusWithMode(Some(Status::Frame{phase:1.0}), Mode::UnlimitedFrames),
            SetNow(0, 500000000),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Frame{phase:(30.0/1.001)-29.0}), Mode::UnlimitedFrames),
            AmountToSleep(0, 500000),
            SetNow(1, 0),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Frame{phase:(60.0/1.001)-59.0}), Mode::UnlimitedFrames),
            SetNow(1, 250000000),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Frame{phase:(75.0/1.001)-74.0}), Mode::UnlimitedFrames),
            SetNow(1, 375000000),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Frame{phase:(82.5/1.001)-82.0}), Mode::UnlimitedFrames),
        ]);
    }
    #[test]
    fn vtvf() {
        run_test((5, 1), 10, &[
            SetNow(0, 400000000),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Frame{phase:1.0}), Mode::UnlimitedFrames),
            SetNow(0, 600000000),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Frame{phase:1.0}), Mode::UnlimitedFrames),
            SetTickrate(10, 1),
            SetNow(0, 800000000),
            // should this be two ticks instead of three? ach...
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Tick), Mode::UnlimitedFrames),
            StatusWithMode(Some(Status::Frame{phase:1.0}), Mode::UnlimitedFrames),
        ]);
    }
    #[test]
    fn pause() {
        run_test((5, 1), 10, &[
            SetNow(0, 400000000),
            StatusWithMode(Some(Status::Tick), Mode::MaxOneFramePerTick),
            StatusWithMode(Some(Status::Frame{phase:1.0}), Mode::MaxOneFramePerTick),
            StatusWithMode(None, Mode::MaxOneFramePerTick),
            SetNow(0, 600000000),
            StatusWithMode(Some(Status::Tick), Mode::MaxOneFramePerTick),
            StatusWithMode(Some(Status::Frame{phase:1.0}), Mode::MaxOneFramePerTick),
            StatusWithMode(None, Mode::MaxOneFramePerTick),
            SetPaused(true),
            SetNow(0, 800000000),
            StatusWithMode(Some(Status::Frame{phase:1.0}), Mode::MaxOneFramePerTick),
            StatusWithMode(None, Mode::MaxOneFramePerTick),
            SetNow(1, 0),
            StatusWithMode(Some(Status::Frame{phase:1.0}), Mode::MaxOneFramePerTick),
            StatusWithMode(None, Mode::MaxOneFramePerTick),
            SetPaused(false),
            StatusWithMode(None, Mode::MaxOneFramePerTick),
            SetNow(1, 200000000),
            StatusWithMode(Some(Status::Tick), Mode::MaxOneFramePerTick),
            StatusWithMode(Some(Status::Frame{phase:1.0}), Mode::MaxOneFramePerTick),
            StatusWithMode(None, Mode::MaxOneFramePerTick),
        ]);
    }
}
