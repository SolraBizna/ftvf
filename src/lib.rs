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
//! Bonus: If you know your refresh rate, `ftvf` can help you render frames at
//! exactly that rate, jitter-free.
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
//!   precision. (Nanosecond precision is still used for frame phase
//!   calculation, and changing tick-/framerates at runtime also discards
//!   sub-nanosecond components.)
//! - `Status` has been renamed to `Reading`.
//! - `Reading::Idle` now directly gives you the wait time as a `Duration`,
//!   instead of making you go indirectly through the `metronome`.
//! - `Mode::TargetFramesPerSecond` added.
//! - Tickrate can now be changed at any time, with no temporal anomaly—apart
//!   from up to one nanosecond of one-time temporal error per change.
//! - `NowSource::sleep` removed.
//! - `NowSource` no longer implies `Copy`.
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

mod rate;
#[doc(inline)]
pub use rate::*;
mod residual;
use residual::*;
mod metronome;
#[doc(inline)]
pub use metronome::*;
mod time;
#[doc(inline)]
pub use time::*;

#[cfg(test)]
mod test;