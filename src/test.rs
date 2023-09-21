
#[cfg(feature="no_std")]
use std::prelude::*;

use std::{
    cell::RefCell,
    num::NonZeroU32,
    time::Duration,
};

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
            Reading::Frame { phase: 1.0 },
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
            Reading::Tick,
            Reading::Frame { phase: 0.5 },
        ]),
        Sample(Mode::UnlimitedFrames, &[
        ]),
        SetNow(2, 200000000),
        Sample(Mode::UnlimitedFrames, &[
            Reading::Frame { phase: 1.0 },
        ]),
        SetNow(1, 0),
        Sample(Mode::UnlimitedFrames, &[
            Reading::TimeWentBackwards,
            Reading::Tick,
            Reading::Frame { phase: 1.0 },
        ]),
    ]);
}
#[test]
fn ntsc() {
    run_test((60000, 1001), 120, &[
        Sample(Mode::UnlimitedFrames, &[
            Reading::Tick,
            Reading::Frame { phase: 1.0 },
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
            Reading::Tick,
            Reading::Frame { phase: 0.97002995 }, // roughly 30.0 / 1.001 - 29.0
        ]),
    ]);
}
#[test]
fn marathon() {
    const SIXTY_FPS: Rate = unsafe { Rate::per_second_nonzero(NonZeroU32::new_unchecked(60), NonZeroU32::new_unchecked(1)) };
    run_test((30, 1), 94332, &[
        Sample(Mode::TargetFramesPerSecond(SIXTY_FPS), &[
            Reading::Tick,
            Reading::Frame { phase: 1.0 },
        ]),
        SetNow(0, 1000000000 * 2 / 60),
        Sample(Mode::TargetFramesPerSecond(SIXTY_FPS), &[
            Reading::Tick,
            Reading::Frame { phase: 1.0 },
        ]),
        SetNow(0, 1000000000 * 3 / 60),
        Sample(Mode::TargetFramesPerSecond(SIXTY_FPS), &[
            Reading::Tick,
            Reading::Frame { phase: 0.50000006 },
        ]),
        SetNow(0, 1000000000 * 4 / 60),
        Sample(Mode::TargetFramesPerSecond(SIXTY_FPS), &[
            Reading::Frame { phase: 1.0 },
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
